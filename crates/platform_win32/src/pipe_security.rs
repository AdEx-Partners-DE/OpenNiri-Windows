//! Access control for the daemon's IPC named pipe.
//!
//! The pipe accepts commands that move, hide and close windows, so only the
//! user who runs the daemon (and SYSTEM) may connect. Clients in turn check that
//! the server behind the pipe name belongs to them: another user could create a
//! pipe with the predictable name first and answer `Ok` to `panic-revert`,
//! which would suppress the CLI's local recovery.

use std::ffi::c_void;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER,
};
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::Win32Error;

/// SDDL granting full access to one user SID and SYSTEM, nobody else.
/// `P` protects the DACL from inheritance.
fn pipe_sddl(user_sid: &str) -> String {
    format!("D:P(A;;GA;;;{})(A;;GA;;;SY)", user_sid)
}

/// String SID (`S-1-5-21-...`) of the user that owns a process token.
fn token_user_sid(process: HANDLE) -> windows::core::Result<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token)?;
        let mut len: u32 = 0;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        // u64 storage keeps the TOKEN_USER read aligned.
        let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
        let result = GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr() as *mut c_void),
            len,
            &mut len,
        );
        let _ = CloseHandle(token);
        result?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut sid_string = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut sid_string)?;
        let text = sid_string.to_string().unwrap_or_default();
        let _ = LocalFree(Some(HLOCAL(sid_string.0 as *mut c_void)));
        Ok(text)
    }
}

/// String SID of the user running this process.
pub fn current_user_sid() -> Option<String> {
    token_user_sid(unsafe { GetCurrentProcess() }).ok()
}

/// Security attributes that restrict a named pipe to the current user and SYSTEM.
///
/// Pass [`PipeSecurityAttributes::as_mut_ptr`] to
/// `tokio::net::windows::named_pipe::ServerOptions::create_with_security_attributes_raw`.
pub struct PipeSecurityAttributes {
    descriptor: PSECURITY_DESCRIPTOR,
    attributes: Box<SECURITY_ATTRIBUTES>,
}

// The security descriptor is allocated once and never modified, so the raw
// pointers can be moved to and shared with other threads.
unsafe impl Send for PipeSecurityAttributes {}
unsafe impl Sync for PipeSecurityAttributes {}

impl PipeSecurityAttributes {
    /// Build attributes that allow only the current user and SYSTEM.
    pub fn current_user_only() -> Result<Self, Win32Error> {
        let sid = current_user_sid().ok_or_else(|| {
            Win32Error::PipeSecurityFailed("Could not read the current user's SID".to_string())
        })?;
        let sddl: Vec<u16> = pipe_sddl(&sid).encode_utf16().chain(Some(0)).collect();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(|e| {
            Win32Error::PipeSecurityFailed(format!(
                "Failed to build pipe security descriptor: {}",
                e
            ))
        })?;
        let attributes = Box::new(SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        });
        Ok(Self {
            descriptor,
            attributes,
        })
    }

    /// Pointer to the `SECURITY_ATTRIBUTES`, valid while `self` lives.
    pub fn as_mut_ptr(&mut self) -> *mut c_void {
        &mut *self.attributes as *mut SECURITY_ATTRIBUTES as *mut c_void
    }
}

impl Drop for PipeSecurityAttributes {
    fn drop(&mut self) {
        if !self.descriptor.0.is_null() {
            let _ = unsafe { LocalFree(Some(HLOCAL(self.descriptor.0))) };
        }
    }
}

fn session_of(process_id: u32) -> Option<u32> {
    let mut session = 0u32;
    unsafe { ProcessIdToSessionId(process_id, &mut session) }
        .ok()
        .map(|()| session)
}

/// Check that the server end of a connected pipe belongs to this user.
///
/// Fails closed. The server must run in this logon session, its process token
/// must be readable, and the token's user SID must equal ours. A server whose
/// process or token cannot be opened is refused: a process of another user in
/// this session (for example started with `runas`) denies token access exactly
/// like that, so a denial proves nothing about the owner. An elevated daemon of
/// the same user passes, because Windows lets a medium-integrity process of
/// that user query the elevated token (`TOKEN_QUERY`; verified on Windows 11).
pub fn verify_pipe_server(pipe_handle: isize) -> Result<(), String> {
    let handle = HANDLE(pipe_handle as *mut c_void);
    let mut server_pid = 0u32;
    unsafe { GetNamedPipeServerProcessId(handle, &mut server_pid) }
        .map_err(|e| format!("cannot identify the pipe server process: {}", e))?;

    let own_session = session_of(unsafe { GetCurrentProcessId() });
    let server_session = session_of(server_pid);
    if own_session.is_none() || own_session != server_session {
        return Err(format!(
            "pipe server process {} runs in another logon session ({:?}, ours {:?})",
            server_pid, server_session, own_session
        ));
    }

    let own_sid =
        current_user_sid().ok_or_else(|| "cannot read this process's user SID".to_string())?;
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, server_pid) }
        .map_err(|e| format!("cannot open pipe server process {}: {}", server_pid, e))?;
    let server_sid = token_user_sid(process);
    let _ = unsafe { CloseHandle(process) };
    match server_sid {
        Ok(sid) if sid == own_sid => Ok(()),
        Ok(sid) => Err(format!(
            "pipe server process {} belongs to another user ({})",
            server_pid, sid
        )),
        Err(e) => Err(format!(
            "cannot verify the owner of pipe server process {}: {}",
            server_pid, e
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe_sddl_allows_only_user_and_system() {
        assert_eq!(
            pipe_sddl("S-1-5-21-1-2-3-1001"),
            "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)(A;;GA;;;SY)"
        );
    }

    #[test]
    fn test_current_user_sid_and_security_attributes() {
        let sid = current_user_sid().expect("current user SID");
        assert!(sid.starts_with("S-1-"), "unexpected SID {}", sid);
        let mut attrs = PipeSecurityAttributes::current_user_only().expect("attributes");
        assert!(!attrs.as_mut_ptr().is_null());
    }
}
