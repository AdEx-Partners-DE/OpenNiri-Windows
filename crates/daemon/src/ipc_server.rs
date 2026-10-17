//! Named-pipe IPC server and the single-instance check.

use crate::DaemonEvent;
use anyhow::Result;
use openniri_ipc::{
    pipe_name_candidates, preferred_pipe_name, IpcCommand, IpcResponse, MAX_IPC_MESSAGE_SIZE,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tracing::{debug, error, warn};

/// IPC read timeout - clients must send within this period.
const IPC_READ_TIMEOUT: Duration = Duration::from_secs(5);
/// IPC responder timeout - daemon must answer within this period.
const IPC_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

fn response_for_ipc_wait_failure(cmd: &IpcCommand, timed_out: bool) -> IpcResponse {
    if matches!(cmd, IpcCommand::Stop | IpcCommand::PanicRevert) {
        // Stop/panic_revert semantics are "shutdown initiated"; don't report as a hard failure
        // if the responder channel closes or cleanup outlives the client timeout.
        IpcResponse::Ok
    } else if timed_out {
        IpcResponse::error("Timed out waiting for daemon response")
    } else {
        IpcResponse::error("Failed to get response from daemon")
    }
}

/// Run the IPC server, accepting connections and dispatching commands.
pub(crate) async fn run_ipc_server(event_tx: mpsc::Sender<DaemonEvent>) {
    let mut is_first_instance = true;
    let pipe_name = preferred_pipe_name();
    // Bound concurrent IPC handlers to avoid local task-exhaustion DoS.
    let connection_limit = Arc::new(Semaphore::new(32));
    // Only this user and SYSTEM may connect: the pipe accepts commands that move,
    // hide and close windows.
    let mut pipe_security =
        match openniri_platform_win32::pipe_security::PipeSecurityAttributes::current_user_only() {
            Ok(security) => Some(security),
            Err(e) => {
                warn!(
                    "Could not restrict the IPC pipe to the current user ({}); using the default ACL",
                    e
                );
                None
            }
        };

    loop {
        let permit = match connection_limit.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                warn!("IPC connection limiter closed while accepting client");
                return;
            }
        };

        // Create a new pipe server instance
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(is_first_instance)
            .pipe_mode(PipeMode::Byte);
        let created = match pipe_security.as_mut() {
            // SAFETY: the pointer refers to SECURITY_ATTRIBUTES owned by
            // `pipe_security`, which outlives this call.
            Some(security) => unsafe {
                options.create_with_security_attributes_raw(&pipe_name, security.as_mut_ptr())
            },
            None => options.create(&pipe_name),
        };
        let server = match created {
            Ok(s) => {
                is_first_instance = false; // Subsequent instances don't need this flag
                s
            }
            Err(e) => {
                error!("Failed to create named pipe server: {}", e);
                if is_first_instance {
                    // If we can't create the first instance, maybe another daemon is running
                    error!("Is another openniri daemon already running?");
                }
                drop(permit);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        debug!("Waiting for client connection on {}", pipe_name);

        // Wait for a client to connect
        if let Err(e) = server.connect().await {
            error!("Failed to accept client connection: {}", e);
            drop(permit);
            continue;
        }

        debug!("Client connected");

        // Handle this client
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(server, event_tx).await {
                warn!("Client handler error: {}", e);
            }
            drop(permit);
        });
    }
}

/// Handle a single client connection.
async fn handle_client(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    event_tx: mpsc::Sender<DaemonEvent>,
) -> Result<()> {
    async fn write_ipc_response_line<W>(writer: &mut W, response: &IpcResponse) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut response_json = match serde_json::to_string(response) {
            Ok(json) => json + "\n",
            Err(e) => {
                warn!("Failed to serialize IPC response: {}", e);
                "{\"status\":\"error\",\"message\":\"Internal serialization error\"}\n".to_string()
            }
        };

        if response_json.len() > MAX_IPC_MESSAGE_SIZE {
            warn!(
                "IPC response exceeded {} bytes; returning bounded error response instead",
                MAX_IPC_MESSAGE_SIZE
            );
            response_json = serde_json::to_string(&IpcResponse::error(
                "IPC response exceeded maximum size; narrow query scope and retry",
            ))
            .unwrap_or_else(|_| {
                "{\"status\":\"error\",\"message\":\"Internal serialization error\"}".to_string()
            });
            response_json.push('\n');
        }

        writer.write_all(response_json.as_bytes()).await?;
        Ok(())
    }

    let (reader, mut writer) = tokio::io::split(pipe);
    let limited_reader = reader.take(MAX_IPC_MESSAGE_SIZE as u64);
    let mut reader = BufReader::new(limited_reader);
    let mut line = String::new();

    // Read command (single line of JSON) with timeout and size bound
    let read_result = tokio::time::timeout(IPC_READ_TIMEOUT, reader.read_line(&mut line)).await;
    let bytes_read = match read_result {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => {
            // Timeout: client did not send in time, silently close
            return Ok(());
        }
    };
    if bytes_read == 0 {
        return Ok(()); // Client disconnected
    }

    if !line.ends_with('\n') {
        let msg = if bytes_read >= MAX_IPC_MESSAGE_SIZE {
            "Command too large or missing newline terminator"
        } else {
            "IPC command must be newline-terminated"
        };
        write_ipc_response_line(&mut writer, &IpcResponse::error(msg)).await?;
        return Ok(());
    }

    let line = line.trim_end_matches(['\r', '\n']);
    debug!("Received command: {}", line);

    // Parse the command
    let cmd: IpcCommand = match serde_json::from_str(line) {
        Ok(cmd) => cmd,
        Err(e) => {
            let response = IpcResponse::error(format!("Invalid command: {}", e));
            write_ipc_response_line(&mut writer, &response).await?;
            return Ok(());
        }
    };

    // Create a oneshot channel for the response
    let (resp_tx, resp_rx) = oneshot::channel();
    let response_cmd = cmd.clone();

    // Send the command to the event loop
    if event_tx
        .send(DaemonEvent::IpcCommand {
            cmd,
            responder: resp_tx,
        })
        .await
        .is_err()
    {
        let response = IpcResponse::error("Daemon is shutting down");
        write_ipc_response_line(&mut writer, &response).await?;
        return Ok(());
    }

    // Wait for the response (bounded so clients don't hang forever).
    let response = match tokio::time::timeout(IPC_RESPONSE_TIMEOUT, resp_rx).await {
        Ok(Ok(resp)) => resp,
        Ok(Err(_)) => response_for_ipc_wait_failure(&response_cmd, false),
        Err(_) => response_for_ipc_wait_failure(&response_cmd, true),
    };

    // Send response back to client.
    write_ipc_response_line(&mut writer, &response).await?;

    Ok(())
}

const ERROR_PIPE_BUSY: i32 = 231;
const ERROR_FILE_NOT_FOUND: i32 = 2;

fn pipe_probe_error_indicates_running(error: &std::io::Error) -> bool {
    match error.raw_os_error() {
        Some(ERROR_PIPE_BUSY) => true,
        Some(ERROR_FILE_NOT_FOUND) => false,
        _ if error.kind() == std::io::ErrorKind::NotFound => false,
        _ => true,
    }
}

fn pipe_probe_result_indicates_running<T>(probe_result: std::io::Result<T>) -> bool {
    match probe_result {
        Ok(_) => true,
        Err(error) => pipe_probe_error_indicates_running(&error),
    }
}

/// Check if another daemon instance is already running by probing the named pipe.
///
/// Returns `true` if the pipe exists (connected or busy). ERROR_PIPE_BUSY (231)
/// means another client is already connected — the daemon is still running.
pub(crate) async fn check_already_running() -> bool {
    for pipe_name in pipe_name_candidates() {
        let probe_result = tokio::net::windows::named_pipe::ClientOptions::new().open(&pipe_name);
        if let Err(error) = &probe_result {
            if pipe_probe_error_indicates_running(error)
                && error.raw_os_error() != Some(ERROR_PIPE_BUSY)
            {
                warn!(
                    "Named pipe probe for {} failed with non-NotFound error ({}); assuming daemon is already running to avoid duplicate instances",
                    pipe_name,
                    error
                );
            }
        }
        if pipe_probe_result_indicates_running(probe_result.map(|_| ())) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery::{shutdown_mode_for_command, ShutdownMode};

    #[test]
    fn test_ipc_wait_failure_responses_and_shutdown_mapping() {
        // Stop/PanicRevert mean "shutdown initiated": a closed responder is not a failure.
        for timed_out in [true, false] {
            assert_eq!(
                response_for_ipc_wait_failure(&IpcCommand::Stop, timed_out),
                IpcResponse::Ok
            );
            assert_eq!(
                response_for_ipc_wait_failure(&IpcCommand::PanicRevert, timed_out),
                IpcResponse::Ok
            );
        }
        assert!(matches!(
            response_for_ipc_wait_failure(&IpcCommand::FocusLeft, true),
            IpcResponse::Error { message } if message.contains("Timed out")
        ));
        assert!(matches!(
            response_for_ipc_wait_failure(&IpcCommand::FocusLeft, false),
            IpcResponse::Error { message } if message.contains("Failed to get response")
        ));

        assert_eq!(
            shutdown_mode_for_command(&IpcCommand::Stop),
            Some(ShutdownMode::Graceful)
        );
        assert_eq!(
            shutdown_mode_for_command(&IpcCommand::PanicRevert),
            Some(ShutdownMode::PanicRevert)
        );
        assert_eq!(shutdown_mode_for_command(&IpcCommand::FocusLeft), None);
    }

    #[test]
    fn test_check_already_running_with_isolated_pipe() {
        // Error classification: busy/denied mean another daemon owns the pipe.
        let busy = std::io::Error::from_raw_os_error(ERROR_PIPE_BUSY);
        assert!(pipe_probe_error_indicates_running(&busy));
        let access_denied = std::io::Error::from_raw_os_error(5); // ERROR_ACCESS_DENIED
        assert!(pipe_probe_error_indicates_running(&access_denied));
        let not_found = std::io::Error::from_raw_os_error(ERROR_FILE_NOT_FOUND);
        assert!(!pipe_probe_error_indicates_running(&not_found));
        let not_found_kind = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        assert!(!pipe_probe_error_indicates_running(&not_found_kind));

        // Real probe against an isolated pipe name, independent of a running daemon.
        let pipe_name = format!(r"\\.\pipe\openniri-test-singleton-{}", std::process::id());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();
        let result = rt.block_on(async {
            pipe_probe_result_indicates_running(
                tokio::net::windows::named_pipe::ClientOptions::new()
                    .open(&pipe_name)
                    .map(|_| ()),
            )
        });
        assert!(
            !result,
            "No pipe server exists, so connect should fail (no daemon)"
        );
    }
}
