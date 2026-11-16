//! Typed retry rejections crossing to the native exception translators.
use serde::Serialize;
use status_machina::{
    RetryPolicyBuildError,
    RetryWaitError,
};

/// A kernel retry rejection, or a native value the kernel types cannot hold.
///
/// `family` is `policy` (`RetryPolicyBuildError`), `waits` (`RetryWaitError`),
/// `range` (a value outside its kernel type, named by `field`) or `transport`
/// (malformed private transport, never caused by application values).
#[derive(Debug, Serialize)]
pub struct Failure {
    pub code: &'static str,
    pub family: &'static str,
    pub reason: &'static str,
    pub message: String,
    pub field: Option<&'static str>,
    pub limit: Option<usize>,
}
impl Failure {
    fn new(family: &'static str, reason: &'static str, message: impl ToString) -> Self {
        Self {
            code: "retry",
            family,
            reason,
            message: message.to_string(),
            field: None,
            limit: None,
        }
    }
    pub(crate) fn policy(error: RetryPolicyBuildError) -> Self {
        let reason = match error {
            RetryPolicyBuildError::ZeroDelay => "zero_delay",
            RetryPolicyBuildError::TooFewAttempts => "too_few_attempts",
            RetryPolicyBuildError::CapBelowInitial => "cap_below_initial",
        };
        Self::new("policy", reason, error)
    }
    pub(crate) fn waits(error: RetryWaitError, capacity: Option<usize>) -> Self {
        let reason = match error {
            RetryWaitError::InvalidCapacity => "invalid_capacity",
            RetryWaitError::CapacityReached => "capacity_reached",
            RetryWaitError::GenerationExhausted => "generation_exhausted",
        };
        let mut failure = Self::new("waits", reason, error);
        failure.limit = capacity;
        failure
    }
    pub(crate) fn range(field: &'static str, message: impl ToString) -> Self {
        let mut failure = Self::new("range", "out_of_range", message);
        failure.field = Some(field);
        failure
    }
    pub(crate) fn transport(error: impl ToString) -> Self {
        Self::new("transport", "malformed", error)
    }
    /// Serialize for a native exception translator.
    #[must_use]
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("typed retry failure metadata is serializable")
    }
}

/// Parse one private transport value.
pub(crate) fn parse<T: serde::de::DeserializeOwned>(source: &str) -> Result<T, Failure> {
    serde_json::from_str(source).map_err(Failure::transport)
}
