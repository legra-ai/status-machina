//! Typed errors translated to each language's public exception hierarchy.
use serde::Serialize;
use status_machina::{
    DefinitionBuildError,
    DispatchError,
    EvaluationError,
    EvaluationPhase,
};

use crate::ids::Ids;
use crate::value::Host;

#[derive(Debug, thiserror::Error)]
pub enum Error<H: Host> {
    #[error("invalid definition: {0}")]
    Definition(DefinitionBuildError<Ids<H>>),
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
}
#[derive(Serialize)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
    pub phase: Option<String>,
    pub state: Option<u32>,
    pub condition: Option<u32>,
    pub expected_revision: Option<String>,
    pub current_revision: Option<String>,
}
impl<H: Host> Error<H> {
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
            expected_revision: None,
            current_revision: None,
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
            Self::Definition(_) | Self::Construction { .. } => {}
        }
        f
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
