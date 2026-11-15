//! Typed errors translated to each language's public exception hierarchy.
use serde::Serialize;
use status_machina::{
    BehaviorRole,
    BindingError,
    DefinitionBuildError,
    DispatchError,
    EvaluationError,
    EvaluationPhase,
    RecordBuildError,
};

use crate::ids::Ids;
use crate::value::Host;

#[derive(Debug, thiserror::Error)]
pub enum Error<H: Host> {
    #[error("invalid definition: {0}")]
    Definition(DefinitionBuildError<Ids<H>>),
    #[error("invalid definition record: {0}")]
    Record(RecordBuildError<Ids<H>>),
    #[error("invalid {role}: {message}")]
    Construction { role: &'static str, message: String },
    #[error(transparent)]
    Dispatch(DispatchError<Ids<H>>),
    #[error("{phase:?} evaluation failed: {error}")]
    Evaluation {
        phase: EvaluationPhase,
        error: EvaluationError,
    },
    #[error("native resource has been disposed")]
    Disposed,
    #[error("native resource is already executing an operation")]
    Busy,
    #[error("inspection failed: {message}")]
    Inspection {
        message: String,
        limit: Option<usize>,
    },
}
#[derive(Serialize)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
    pub phase: Option<String>,
    pub state: Option<u32>,
    pub condition: Option<u32>,
    pub transition: Option<u32>,
    pub disposition: Option<u32>,
    pub story: Option<u32>,
    pub implementation: Option<u32>,
    pub expected_role: Option<&'static str>,
    pub actual_role: Option<&'static str>,
    pub expected_revision: Option<String>,
    pub current_revision: Option<String>,
    pub limit: Option<usize>,
}
impl<H: Host> Error<H> {
    pub fn invalid_inspection(error: impl std::fmt::Display) -> Self {
        Self::Inspection {
            message: error.to_string(),
            limit: None,
        }
    }
    pub fn inspection(error: status_machina::InspectionError) -> Self {
        let limit = match error {
            status_machina::InspectionError::EntryLimit { limit }
            | status_machina::InspectionError::OutputLimit { limit } => Some(limit),
            _ => None,
        };
        Self::Inspection {
            message: error.to_string(),
            limit,
        }
    }
    pub fn construction(role: &'static str, error: impl std::fmt::Display) -> Self {
        Self::Construction {
            role,
            message: error.to_string(),
        }
    }
    pub fn evaluation(error: EvaluationError) -> Self {
        Self::Evaluation {
            phase: EvaluationPhase::Snapshot,
            error,
        }
    }
    pub fn cause(&self) -> Option<&EvaluationError> {
        match self {
            Self::Evaluation { error, .. }
            | Self::Dispatch(DispatchError::EvaluationFailed { error, .. })
            | Self::Definition(DefinitionBuildError::EvaluationFailed { error }) => Some(error),
            _ => None,
        }
    }
    pub fn failure(&self) -> Failure {
        let mut f = Failure {
            code: "definition",
            message: self.to_string(),
            phase: None,
            state: None,
            condition: None,
            transition: None,
            disposition: None,
            story: None,
            implementation: None,
            expected_role: None,
            actual_role: None,
            expected_revision: None,
            current_revision: None,
            limit: None,
        };
        match self {
            Self::Evaluation { phase, .. }
            | Self::Dispatch(DispatchError::EvaluationFailed { phase, .. }) => {
                f.code = "callback";
                f.phase = Some(format!("{phase:?}").to_lowercase());
            }
            Self::Definition(DefinitionBuildError::EvaluationFailed { .. }) => {
                f.code = "callback";
                f.phase = Some("invariant".into());
            }
            Self::Dispatch(DispatchError::InvariantFailed { state }) => {
                f.code = "invariant";
                f.state = Some(*state);
            }
            Self::Dispatch(DispatchError::PostconditionFailed { condition }) => {
                f.code = "postcondition";
                f.condition = Some(*condition);
            }
            Self::Dispatch(DispatchError::Ambiguous { state }) => {
                f.code = "ambiguous";
                f.state = Some(*state);
            }
            Self::Dispatch(DispatchError::ActionFailed { .. }) => f.code = "action",
            Self::Dispatch(DispatchError::StaleRevision { expected, current }) => {
                f.code = "stale_revision";
                f.expected_revision = Some(expected.to_string());
                f.current_revision = Some(current.to_string());
            }
            Self::Dispatch(DispatchError::DefinitionMismatch { .. }) => {
                f.code = "foreign_definition"
            }
            Self::Dispatch(DispatchError::InstanceMismatch { .. }) => f.code = "foreign_instance",
            Self::Dispatch(DispatchError::RevisionExhausted) => f.code = "revision_exhausted",
            Self::Disposed => f.code = "disposed",
            Self::Busy => f.code = "busy",
            Self::Inspection { limit, .. } => {
                f.code = "inspection";
                f.limit = *limit;
            }
            Self::Record(error) => record(error, &mut f),
            Self::Definition(error) => definition(error, &mut f),
            Self::Construction { .. } => {}
        }
        f
    }
}
/// Native wire name of a Story signature class.
const fn role(value: BehaviorRole) -> &'static str {
    match value {
        BehaviorRole::Action => "action",
        BehaviorRole::Condition => "condition",
        BehaviorRole::Invariant => "invariant",
        BehaviorRole::Effect => "effect",
        BehaviorRole::OutputEvent => "output",
        BehaviorRole::Capability => "capability",
        BehaviorRole::Assertion => "assertion",
    }
}
fn record<H: Host>(error: &RecordBuildError<Ids<H>>, f: &mut Failure) {
    f.code = "record";
    match error {
        RecordBuildError::Binding(error) => binding(error, f),
        RecordBuildError::Definition(error) => definition(error, f),
        RecordBuildError::Condition { id, .. } => {
            f.code = "record_condition";
            f.condition = Some(*id);
        }
        RecordBuildError::Transition { id, .. } => {
            f.code = "record_transition";
            f.transition = Some(*id);
        }
        RecordBuildError::Disposition { id, .. } => {
            f.code = "record_disposition";
            f.disposition = Some(*id);
        }
    }
}
fn binding<H: Host>(error: &BindingError<Ids<H>>, f: &mut Failure) {
    match error {
        BindingError::UnknownBehavior { behavior } => {
            f.code = "unknown_behavior";
            f.story = Some(*behavior);
        }
        BindingError::UnknownImplementation { reference } => {
            f.code = "unknown_implementation";
            f.story = Some(*reference.behavior());
            f.implementation = Some(*reference.implementation());
        }
        BindingError::SignatureMismatch {
            reference,
            expected,
            actual,
        } => {
            f.code = "role_mismatch";
            f.story = Some(*reference.behavior());
            f.implementation = Some(*reference.implementation());
            f.expected_role = Some(role(*expected));
            f.actual_role = Some(role(*actual));
        }
        BindingError::InvalidLimit
        | BindingError::LimitBelowCurrentSize
        | BindingError::LimitExceeded { .. }
        | BindingError::DuplicateImplementation { .. } => {}
    }
}
/// Attach the private role-index context of a graph error, for native lookup.
fn definition<H: Host>(error: &DefinitionBuildError<Ids<H>>, f: &mut Failure) {
    match error {
        DefinitionBuildError::StateEffectLimitExceeded { state }
        | DefinitionBuildError::UnknownInitialState { state }
        | DefinitionBuildError::DuplicateState { state }
        | DefinitionBuildError::UnreachableState { state }
        | DefinitionBuildError::TerminalHasOutgoingTransition { state }
        | DefinitionBuildError::StateActionLimitExceeded { state }
        | DefinitionBuildError::InitialInvariantFailed { state } => f.state = Some(*state),
        DefinitionBuildError::DuplicateTransition { transition } => {
            f.transition = Some(*transition);
        }
        DefinitionBuildError::DuplicateDisposition { disposition } => {
            f.disposition = Some(*disposition);
        }
        DefinitionBuildError::UnknownTransitionState { transition, state } => {
            f.transition = Some(*transition);
            f.state = Some(*state);
        }
        DefinitionBuildError::UnknownDispositionState { disposition, state } => {
            f.disposition = Some(*disposition);
            f.state = Some(*state);
        }
        DefinitionBuildError::StateLimitExceeded { limit }
        | DefinitionBuildError::TransitionLimitExceeded { limit }
        | DefinitionBuildError::DispositionLimitExceeded { limit } => f.limit = Some(*limit),
        DefinitionBuildError::EvaluationFailed { .. }
        | DefinitionBuildError::MissingInitialState
        | DefinitionBuildError::EmptyLimits
        | DefinitionBuildError::LimitsBelowCurrentSize => {}
    }
}
impl<H: Host> From<DefinitionBuildError<Ids<H>>> for Error<H> {
    fn from(error: DefinitionBuildError<Ids<H>>) -> Self {
        Self::Definition(error)
    }
}
impl<H: Host> From<DispatchError<Ids<H>>> for Error<H> {
    fn from(error: DispatchError<Ids<H>>) -> Self {
        Self::Dispatch(error)
    }
}
