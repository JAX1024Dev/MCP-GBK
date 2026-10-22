use std::{
    io,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Control {
    pub token: CancellationToken,
    pub deadline: Instant,
}

impl Control {
    pub fn new(token: CancellationToken, timeout_ms: u64) -> Self {
        Self {
            token,
            deadline: Instant::now() + Duration::from_millis(timeout_ms),
        }
    }

    pub fn check(&self) -> io::Result<()> {
        if self.token.is_cancelled() {
            // Interrupted is automatically retried by many buffered readers.
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "request cancelled",
            ))
        } else if Instant::now() >= self.deadline {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "query time limit reached",
            ))
        } else {
            Ok(())
        }
    }
}
