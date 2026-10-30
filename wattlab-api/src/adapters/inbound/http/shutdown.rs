use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::Notify;

/// Ends long-lived responses (long polls, SSE) so graceful shutdown cannot be
/// held open by a connected screen or agent.
#[derive(Clone, Default)]
pub struct Shutdown(Arc<Inner>);

#[derive(Default)]
struct Inner {
    closing: AtomicBool,
    notify: Notify,
}

impl Shutdown {
    pub fn trigger(&self) {
        self.0.closing.store(true, Ordering::SeqCst);
        self.0.notify.notify_waiters();
    }

    pub async fn wait(&self) {
        loop {
            let notified = self.0.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.0.closing.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }
}
