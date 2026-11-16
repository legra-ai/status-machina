//! Durations and civil instants as decimal seconds plus nanoseconds.
use std::time::{
    Duration,
    SystemTime,
};

use serde::{
    Deserialize,
    Serialize,
};

use super::failure::Failure;

const NANOS_PER_SECOND: u32 = 1_000_000_000;

fn nanoseconds(value: u32, field: &'static str) -> Result<u32, Failure> {
    if value >= NANOS_PER_SECOND {
        return Err(Failure::range(
            field,
            "subsecond nanoseconds must be below one second",
        ));
    }
    Ok(value)
}

/// A non-negative Rust `Duration`: whole seconds (decimal `u64`) plus
/// nanoseconds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurationValue {
    pub seconds: String,
    pub nanoseconds: u32,
}
impl DurationValue {
    /// The Rust duration, or a range failure naming `field`.
    ///
    /// # Errors
    /// Rejects negative or non-integral seconds, seconds beyond `u64` and
    /// nanoseconds of a second or more.
    pub fn to_duration(&self, field: &'static str) -> Result<Duration, Failure> {
        let seconds = self.seconds.parse::<u64>().map_err(|_| {
            Failure::range(
                field,
                "a duration must be non-negative and at most u64::MAX seconds",
            )
        })?;
        Ok(Duration::new(
            seconds,
            nanoseconds(self.nanoseconds, field)?,
        ))
    }
    #[must_use]
    pub fn from_duration(duration: Duration) -> Self {
        Self {
            seconds: duration.as_secs().to_string(),
            nanoseconds: duration.subsec_nanos(),
        }
    }
}

/// A civil instant: whole seconds from the Unix epoch (decimal, floored, may be
/// negative) plus non-negative nanoseconds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstantValue {
    pub seconds: String,
    pub nanoseconds: u32,
}
impl InstantValue {
    /// The platform `SystemTime`, or a range failure naming `field`.
    ///
    /// # Errors
    /// Rejects instants the platform's `SystemTime` cannot represent.
    pub fn to_system_time(&self, field: &'static str) -> Result<SystemTime, Failure> {
        let unrepresentable =
            || Failure::range(field, "the instant is outside this platform's SystemTime");
        let seconds = self
            .seconds
            .parse::<i128>()
            .map_err(|_| unrepresentable())?;
        let nanos = Duration::from_nanos(u64::from(nanoseconds(self.nanoseconds, field)?));
        let whole = Duration::from_secs(
            u64::try_from(seconds.unsigned_abs()).map_err(|_| unrepresentable())?,
        );
        let base = if seconds >= 0 {
            SystemTime::UNIX_EPOCH.checked_add(whole)
        } else {
            SystemTime::UNIX_EPOCH.checked_sub(whole)
        };
        base.and_then(|time| time.checked_add(nanos))
            .ok_or_else(unrepresentable)
    }
    #[must_use]
    pub fn from_system_time(time: SystemTime) -> Self {
        let (seconds, nanoseconds) = match time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(after) => (i128::from(after.as_secs()), after.subsec_nanos()),
            Err(before) => {
                let before = before.duration();
                let seconds = -i128::from(before.as_secs());
                match before.subsec_nanos() {
                    0 => (seconds, 0),
                    nanos => (seconds - 1, NANOS_PER_SECOND - nanos),
                }
            }
        };
        Self {
            seconds: seconds.to_string(),
            nanoseconds,
        }
    }
}
