//! Typed scenario rejections crossing to the native exception translators.
use serde::Serialize;
use status_machina::{
    BehaviorRole,
    BindingError,
    ScenarioAssertionBuildError,
    ScenarioLimitsBuildError,
    ScenarioSetBuildError,
};

use crate::error::Error;
use crate::value::Host;

/// A kernel scenario rejection with its native lookup context.
///
/// `family` names the kernel error type: `limits` (`ScenarioLimitsBuildError`),
/// `set` (`ScenarioSetBuildError`), `assertion` (`ScenarioAssertionBuildError`
/// or a `BindingError` of an assertion reference), `replay`
/// (`ScenarioReplayBuildError`) or `transport` (malformed private transport,
/// never caused by application values). `entry` is the position of the
/// refused scenario or assertion.
#[derive(Debug, Serialize)]
pub struct Failure {
    pub code: &'static str,
    pub family: &'static str,
    pub reason: &'static str,
    pub message: String,
    pub limit: Option<usize>,
    pub entry: Option<usize>,
    pub expected_role: Option<Role>,
    pub actual_role: Option<Role>,
}
/// Native wire name of a Story signature class.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Action,
    Condition,
    Invariant,
    Effect,
    Output,
    Capability,
    Assertion,
}
impl From<BehaviorRole> for Role {
    fn from(value: BehaviorRole) -> Self {
        match value {
            BehaviorRole::Action => Self::Action,
            BehaviorRole::Condition => Self::Condition,
            BehaviorRole::Invariant => Self::Invariant,
            BehaviorRole::Effect => Self::Effect,
            BehaviorRole::OutputEvent => Self::Output,
            BehaviorRole::Capability => Self::Capability,
            BehaviorRole::Assertion => Self::Assertion,
        }
    }
}
impl Failure {
    fn new(family: &'static str, reason: &'static str, message: impl ToString) -> Self {
        Self {
            code: "scenario",
            family,
            reason,
            message: message.to_string(),
            limit: None,
            entry: None,
            expected_role: None,
            actual_role: None,
        }
    }
    pub(crate) fn limits(error: ScenarioLimitsBuildError) -> Self {
        match error {
            ScenarioLimitsBuildError::OutsideSupportedRange => {
                Self::new("limits", "outside_supported_range", error)
            }
        }
    }
    pub(crate) fn set(entry: Option<usize>, error: ScenarioSetBuildError) -> Self {
        let (reason, limit) = match error {
            ScenarioSetBuildError::Empty => ("empty", None),
            ScenarioSetBuildError::ScenarioLimitExceeded { limit } => {
                ("scenario_limit_exceeded", Some(limit))
            }
            ScenarioSetBuildError::StepLimitExceeded { limit } => {
                ("step_limit_exceeded", Some(limit))
            }
            ScenarioSetBuildError::TotalNodeLimitExceeded { limit } => {
                ("total_node_limit_exceeded", Some(limit))
            }
        };
        let mut failure = Self::new("set", reason, error);
        failure.limit = limit;
        failure.entry = entry;
        failure
    }
    pub(crate) fn assertion(entry: Option<usize>, error: ScenarioAssertionBuildError) -> Self {
        let mut failure = match error {
            ScenarioAssertionBuildError::MissingPredicate => {
                Self::new("assertion", "missing_predicate", error)
            }
        };
        failure.entry = entry;
        failure
    }
    pub(crate) fn binding<I: status_machina::MachineIds>(
        entry: usize,
        error: &BindingError<I>,
    ) -> Self {
        let (reason, roles) = match error {
            BindingError::UnknownBehavior { .. } => ("unknown_behavior", None),
            BindingError::UnknownImplementation { .. } => ("unknown_implementation", None),
            BindingError::SignatureMismatch {
                expected, actual, ..
            } => (
                "role_mismatch",
                Some((Role::from(*expected), Role::from(*actual))),
            ),
            BindingError::InvalidLimit
            | BindingError::LimitBelowCurrentSize
            | BindingError::LimitExceeded { .. }
            | BindingError::DuplicateImplementation { .. } => ("registry", None),
        };
        let mut failure = Self::new("assertion", reason, error);
        failure.entry = Some(entry);
        if let Some((expected, actual)) = roles {
            failure.expected_role = Some(expected);
            failure.actual_role = Some(actual);
        }
        failure
    }
    pub(crate) fn assertion_limit(limit: usize) -> Self {
        let mut failure = Self::new(
            "replay",
            "assertion_limit_exceeded",
            format!("scenario assertion limit {limit} exceeded"),
        );
        failure.limit = Some(limit);
        failure
    }
    pub(crate) fn transport(error: impl ToString) -> Self {
        Self::new("transport", "malformed", error)
    }
    /// Serialize for a native exception translator.
    #[must_use]
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("typed scenario failure metadata is serializable")
    }
}

/// Parse one private transport value.
pub(crate) fn parse<T: serde::de::DeserializeOwned>(source: &str) -> Result<T, Failure> {
    if source.len() > 16 * 1024 * 1024 {
        return Err(Failure::transport("scenario transport exceeds 16 MiB"));
    }
    serde_json::from_str(source).map_err(Failure::transport)
}

/// Either a kernel scenario rejection or an ordinary bridge error, such as a
/// callback failure with its original cause or an exhausted value arena.
#[derive(Debug)]
pub enum Fault<H: Host> {
    Scenario(Box<Failure>),
    Core(Error<H>),
}
impl<H: Host> From<Failure> for Fault<H> {
    fn from(failure: Failure) -> Self {
        Self::Scenario(Box::new(failure))
    }
}
impl<H: Host> From<Error<H>> for Fault<H> {
    fn from(error: Error<H>) -> Self {
        Self::Core(error)
    }
}
