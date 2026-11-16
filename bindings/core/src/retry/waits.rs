//! A kernel `RetryWaits` over interned operation indexes; wake payloads stay in
//! the language, keyed by the kernel-assigned generation.
use serde::{
    Deserialize,
    Serialize,
};
use status_machina::{
    RetryWaits,
    WaitCorrelation,
    WakeIntent,
};

use super::failure::{
    Failure,
    parse,
};
use super::ids::{
    Owner,
    RetryIds,
};
use super::time::InstantValue;

/// A native correlation lowered against one wait set: whether its instance is
/// the owner's, the interned operation index (absent when never interned) and
/// its decimal `u64` generation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Correlation {
    pub owner: bool,
    pub operation: Option<u32>,
    pub generation: String,
}
impl Correlation {
    fn lower(&self) -> Result<WaitCorrelation<RetryIds>, Failure> {
        let generation = self.generation.parse::<u64>().map_err(|_| {
            Failure::range(
                "generation",
                "a wait generation must be an integer in 0..=u64::MAX",
            )
        })?;
        let owner = if self.owner {
            Owner::This
        } else {
            Owner::Other
        };
        Ok(WaitCorrelation::new(owner, self.operation, generation))
    }
}
/// One accepted wake intent of the owning instance.
#[derive(Debug, Serialize)]
pub struct Intent {
    pub operation: u32,
    pub generation: String,
    pub deadline: InstantValue,
}
impl Intent {
    fn new(intent: &WakeIntent<(), RetryIds>) -> Self {
        let correlation = intent.correlation();
        Self {
            operation: correlation
                .operation()
                .expect("scheduled operations are always interned"),
            generation: correlation.generation().to_string(),
            deadline: InstantValue::from_system_time(intent.deadline()),
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Schedule {
    operation: u32,
    deadline: InstantValue,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observe {
    correlation: Correlation,
    now: Option<InstantValue>,
}
impl Observe {
    fn parse(source: &str) -> Result<(WaitCorrelation<RetryIds>, Option<InstantValue>), Failure> {
        let request: Self = parse(source)?;
        Ok((request.correlation.lower()?, request.now))
    }
}
fn now(value: Option<InstantValue>) -> Result<std::time::SystemTime, Failure> {
    value
        .ok_or_else(|| Failure::transport("the supplied time is missing"))?
        .to_system_time("now")
}

/// One instance's bounded kernel wait set.
#[derive(Clone, Debug)]
pub struct Waits {
    waits: RetryWaits<(), RetryIds>,
    capacity: Option<usize>,
}
impl Waits {
    /// Build through `RetryWaits::builder`; `None` is a capacity the native
    /// value cannot represent, which the kernel rejects as absent.
    ///
    /// # Errors
    /// Returns the kernel's `InvalidCapacity`.
    pub fn try_new(capacity: Option<usize>) -> Result<Self, Failure> {
        let mut builder = RetryWaits::builder(Owner::This);
        if let Some(capacity) = capacity {
            builder = builder.capacity(capacity);
        }
        Ok(Self {
            waits: builder
                .build()
                .map_err(|error| Failure::waits(error, capacity))?,
            capacity,
        })
    }
    /// `RetryWaits::schedule` for `{operation, deadline}`; returns the decimal
    /// generation of the accepted wait.
    ///
    /// # Errors
    /// Returns a range failure for an unrepresentable deadline, otherwise the
    /// kernel's capacity or generation exhaustion, before any change.
    pub fn schedule(&mut self, source: &str) -> Result<String, Failure> {
        let request: Schedule = parse(source)?;
        let deadline = request.deadline.to_system_time("deadline")?;
        self.waits
            .schedule(Some(request.operation), deadline, ())
            .map(|correlation| correlation.generation().to_string())
            .map_err(|error| Failure::waits(error, self.capacity))
    }
    /// Accepted intents whose registration is not acknowledged.
    #[must_use]
    pub fn pending(&self) -> Vec<Intent> {
        // bounded: the kernel holds at most `capacity` (<= 4096) entries.
        self.waits.pending().map(Intent::new).collect()
    }
    /// The active wait of an operation, registered or pending; `None` is an
    /// operation the language never interned.
    #[must_use]
    pub fn get(&self, operation: Option<u32>) -> Option<Intent> {
        self.waits.get(&operation).map(Intent::new)
    }
    /// `RetryWaits::mark_registered` for `{correlation}`.
    ///
    /// # Errors
    /// Returns a range failure for an unrepresentable generation.
    pub fn mark_registered(&mut self, source: &str) -> Result<bool, Failure> {
        let (correlation, _) = Observe::parse(source)?;
        Ok(self.waits.mark_registered(&correlation))
    }
    /// `RetryWaits::is_due` for `{correlation, now}`.
    ///
    /// # Errors
    /// Returns a range failure for an unrepresentable generation or time.
    pub fn is_due(&self, source: &str) -> Result<bool, Failure> {
        let (correlation, time) = Observe::parse(source)?;
        Ok(self.waits.is_due(&correlation, now(time)?))
    }
    /// `RetryWaits::consume` for `{correlation, now}`.
    ///
    /// # Errors
    /// Returns a range failure for an unrepresentable generation or time.
    pub fn consume(&mut self, source: &str) -> Result<Option<Intent>, Failure> {
        let (correlation, time) = Observe::parse(source)?;
        Ok(self
            .waits
            .consume(&correlation, now(time)?)
            .as_ref()
            .map(Intent::new))
    }
    /// `RetryWaits::cancel`; `None` is an operation never interned.
    pub fn cancel(&mut self, operation: Option<u32>) -> Option<Intent> {
        self.waits.cancel(&operation).as_ref().map(Intent::new)
    }
}
