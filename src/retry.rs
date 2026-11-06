use std::time::Duration;

mod waits;
pub use waits::{
    RetryWaitError,
    RetryWaits,
    RetryWaitsBuilder,
    WaitCorrelation,
    WakeIntent,
};

/// Declarative retry decision evaluated using caller-supplied attempt data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrySchedule {
    /// Request a wake after this delay; no timer is created by this crate.
    At(Duration),
    /// The configured attempt or elapsed-time bound has been reached.
    Exhausted,
}

/// Invalid bounded retry configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RetryPolicyBuildError {
    /// A retry delay must be positive.
    #[error("retry delay must be greater than zero")]
    ZeroDelay,
    /// Maximum attempts must include at least the initial operation.
    #[error("maximum attempts must be at least two")]
    TooFewAttempts,
    /// An exponential cap must be at least the initial delay.
    #[error("maximum delay must be at least the initial delay")]
    CapBelowInitial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DelayRule {
    Fixed(Duration),
    Exponential {
        initial: Duration,
        maximum: Duration,
    },
}

/// Bounded fixed or capped exponential retry policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    delay: DelayRule,
    max_attempts: u32,
    max_elapsed: Option<Duration>,
}

impl RetryPolicy {
    /// Start constructing a retry policy.
    #[must_use]
    pub const fn builder() -> RetryPolicyBuilder {
        RetryPolicyBuilder {
            delay: None,
            max_attempts: None,
            max_elapsed: None,
        }
    }

    /// Evaluate the next retry from caller-owned counts and elapsed duration.
    #[must_use]
    pub fn schedule(self, failed_attempts: u32, elapsed: Duration) -> RetrySchedule {
        if failed_attempts == 0 || failed_attempts >= self.max_attempts {
            return RetrySchedule::Exhausted;
        }
        let delay = match self.delay {
            DelayRule::Fixed(delay) => delay,
            DelayRule::Exponential { initial, maximum } => {
                let mut delay = initial;
                // bounded: a positive Duration reaches any representable cap
                // within 94 doublings, even from one nanosecond.
                for _ in 1..failed_attempts {
                    if delay == maximum {
                        break;
                    }
                    delay = delay.checked_mul(2).unwrap_or(maximum).min(maximum);
                }
                delay
            }
        };
        if self.max_elapsed.is_some_and(|limit| {
            elapsed
                .checked_add(delay)
                .is_none_or(|next_elapsed| next_elapsed > limit)
        }) {
            return RetrySchedule::Exhausted;
        }
        RetrySchedule::At(delay)
    }
}

/// Builder for validated retry limits and delay rules.
pub struct RetryPolicyBuilder {
    delay: Option<DelayRule>,
    max_attempts: Option<u32>,
    max_elapsed: Option<Duration>,
}

impl RetryPolicyBuilder {
    /// Retry after a constant positive delay.
    #[must_use]
    pub const fn fixed_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(DelayRule::Fixed(delay));
        self
    }

    /// Retry with doubling delay, capped at `maximum`.
    #[must_use]
    pub const fn exponential_backoff(mut self, initial: Duration, maximum: Duration) -> Self {
        self.delay = Some(DelayRule::Exponential { initial, maximum });
        self
    }

    /// Bound total operation attempts, including the initial attempt.
    #[must_use]
    pub const fn max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = Some(attempts);
        self
    }

    /// Optionally bound elapsed time from the operation's first attempt.
    #[must_use]
    pub const fn max_elapsed(mut self, elapsed: Duration) -> Self {
        self.max_elapsed = Some(elapsed);
        self
    }

    /// Validate and build the policy.
    ///
    /// # Errors
    ///
    /// Returns an error when delay or attempt limits are absent or invalid.
    pub fn build(self) -> Result<RetryPolicy, RetryPolicyBuildError> {
        let delay = self.delay.ok_or(RetryPolicyBuildError::ZeroDelay)?;
        let (initial, maximum) = match delay {
            DelayRule::Fixed(value) => (value, value),
            DelayRule::Exponential { initial, maximum } => (initial, maximum),
        };
        if initial.is_zero() {
            return Err(RetryPolicyBuildError::ZeroDelay);
        }
        if maximum < initial {
            return Err(RetryPolicyBuildError::CapBelowInitial);
        }
        let max_attempts = self
            .max_attempts
            .ok_or(RetryPolicyBuildError::TooFewAttempts)?;
        if max_attempts < 2 {
            return Err(RetryPolicyBuildError::TooFewAttempts);
        }
        Ok(RetryPolicy {
            delay,
            max_attempts,
            max_elapsed: self.max_elapsed,
        })
    }
}
