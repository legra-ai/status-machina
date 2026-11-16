//! A checked kernel `RetryPolicy` evaluated from caller-supplied counts and
//! elapsed time; no clock is read.
use serde::{
    Deserialize,
    Serialize,
};
use status_machina::{
    RetryPolicy,
    RetrySchedule,
};

use super::failure::{
    Failure,
    parse,
};
use super::time::DurationValue;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
enum Delay {
    #[serde(rename = "fixed")]
    Fixed { delay: DurationValue },
    #[serde(rename = "exponential")]
    Exponential {
        initial: DurationValue,
        maximum: DurationValue,
    },
}
/// Absent `max_elapsed` is an explicitly indefinite elapsed window.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    delay: Delay,
    max_attempts: String,
    max_elapsed: Option<DurationValue>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    failed_attempts: String,
    elapsed: DurationValue,
}
/// The kernel decision: retry after `delay`, or exhausted.
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum Scheduled {
    #[serde(rename = "at")]
    At { delay: DurationValue },
    #[serde(rename = "exhausted")]
    Exhausted,
}

fn attempts(value: &str, field: &'static str) -> Result<u32, Failure> {
    value
        .parse()
        .map_err(|_| Failure::range(field, "an attempt count must be an integer in 0..=u32::MAX"))
}

/// One kernel-validated retry policy.
#[derive(Clone, Copy, Debug)]
pub struct Policy(RetryPolicy);
impl Policy {
    /// Build through `RetryPolicy::builder`.
    ///
    /// # Errors
    /// Returns a range failure for unrepresentable values, otherwise the
    /// kernel's `RetryPolicyBuildError`.
    pub fn parse(source: &str) -> Result<Self, Failure> {
        let spec: Spec = parse(source)?;
        let builder = match spec.delay {
            Delay::Fixed { delay } => {
                RetryPolicy::builder().fixed_delay(delay.to_duration("delay")?)
            }
            Delay::Exponential { initial, maximum } => RetryPolicy::builder().exponential_backoff(
                initial.to_duration("initial")?,
                maximum.to_duration("maximum")?,
            ),
        };
        let mut builder = builder.max_attempts(attempts(&spec.max_attempts, "max_attempts")?);
        if let Some(window) = spec.max_elapsed {
            builder = builder.max_elapsed(window.to_duration("max_elapsed")?);
        }
        builder.build().map(Self).map_err(Failure::policy)
    }
    /// Evaluate `RetryPolicy::schedule` for `{failed_attempts, elapsed}`.
    ///
    /// # Errors
    /// Returns a range failure for unrepresentable inputs.
    pub fn schedule(&self, source: &str) -> Result<Scheduled, Failure> {
        let request: Request = parse(source)?;
        let failed = attempts(&request.failed_attempts, "failed_attempts")?;
        let elapsed = request.elapsed.to_duration("elapsed")?;
        Ok(match self.0.schedule(failed, elapsed) {
            RetrySchedule::At(delay) => Scheduled::At {
                delay: DurationValue::from_duration(delay),
            },
            RetrySchedule::Exhausted => Scheduled::Exhausted,
        })
    }
}
