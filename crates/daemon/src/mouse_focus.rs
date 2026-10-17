//! Cancellation-safe focus-follows-mouse debounce owned by the event loop.

use std::time::Duration;
use tokio::time::Instant;

#[derive(Default)]
pub(crate) struct MouseFocusDelay {
    pending: Option<(u64, Instant)>,
}

impl MouseFocusDelay {
    pub(crate) fn clear(&mut self) {
        self.pending = None;
    }

    pub(crate) fn schedule(&mut self, window_id: u64, delay: Duration) {
        self.pending = Some((window_id, Instant::now() + delay));
    }

    /// Safe to cancel in `select!`: only consume the request after its deadline.
    /// Keeping the deadline here avoids stale timer messages in the event queue.
    pub(crate) async fn wait(&mut self) -> u64 {
        match self.pending {
            Some((window_id, deadline)) => {
                tokio::time::sleep_until(deadline).await;
                self.pending = None;
                window_id
            }
            None => std::future::pending().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn replacing_a_request_discards_even_an_expired_target() {
        // Includes leaving and re-entering the same window: it needs a fresh delay.
        for (elapsed_ms, next_window) in [(50, 200), (150, 200), (150, 100)] {
            let mut focus = MouseFocusDelay::default();
            focus.schedule(100, Duration::from_millis(100));
            tokio::time::advance(Duration::from_millis(elapsed_ms)).await;
            focus.schedule(next_window, Duration::from_millis(100));
            let scheduled_at = Instant::now();
            assert_eq!(focus.wait().await, next_window);
            assert_eq!(scheduled_at.elapsed(), Duration::from_millis(100));
            assert!(focus.pending.is_none());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn reload_cancels_focus_until_a_new_mouse_transition() {
        let mut focus = MouseFocusDelay::default();
        focus.schedule(100, Duration::from_millis(100));
        focus.clear();
        tokio::select! {
            _ = focus.wait() => panic!("old focus request survived reload"),
            _ = tokio::time::sleep(Duration::from_secs(1)) => {},
        }
        focus.schedule(200, Duration::from_millis(100));
        assert_eq!(focus.wait().await, 200);
    }

    #[tokio::test(start_paused = true)]
    async fn unrelated_events_do_not_consume_or_restart_the_delay() {
        let mut focus = MouseFocusDelay::default();
        focus.schedule(100, Duration::from_millis(100));
        let scheduled_at = Instant::now();
        tokio::select! {
            _ = focus.wait() => panic!("focus must not fire before the deadline"),
            _ = tokio::time::sleep(Duration::from_millis(40)) => {},
        }
        assert_eq!(focus.wait().await, 100);
        assert_eq!(scheduled_at.elapsed(), Duration::from_millis(100));
        tokio::select! {
            _ = focus.wait() => panic!("consumed focus request must not fire again"),
            _ = tokio::time::sleep(Duration::from_secs(1)) => {},
        }
    }
}
