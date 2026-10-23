use buildplane_errors::errors::TimeoutError;
use buildplane_errors::errors::{BpError, BpResult};
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, Copy)]
pub struct Deadline {
    at: Instant,
    buffer: Duration,
}

impl Deadline {
    pub fn new(deadline: SystemTime, buffer: Duration) -> Self {
        let remaining = deadline
            .duration_since(SystemTime::now())
            .unwrap_or_default();
        Self {
            at: Instant::now() + remaining,
            buffer,
        }
    }

    pub fn remaining(self) -> BpResult<Duration> {
        let remaining = self
            .at
            .saturating_duration_since(Instant::now())
            .saturating_sub(self.buffer);
        if remaining.is_zero() {
            return Err(BpError::Timeout(TimeoutError::DeadlineExceeded));
        }
        Ok(remaining)
    }
}
