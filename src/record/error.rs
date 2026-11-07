use crate::{
    BindingError,
    ConditionBuildError,
    DefaultIds,
    DefinitionBuildError,
    DispositionBuildError,
    MachineIds,
    TransitionBuildError,
};

/// A record failed the ordinary binding or builder boundary.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RecordBuildError<I: MachineIds = DefaultIds> {
    /// Exact behavior resolution failed before instantiation.
    #[error(transparent)]
    Binding(#[from] BindingError<I>),
    /// Graph or state construction failed.
    #[error(transparent)]
    Definition(#[from] DefinitionBuildError<I>),
    /// A condition failed structural validation.
    #[error("condition {id:?}: {source}")]
    Condition {
        /// Condition containing the invalid structure.
        id: I::Condition,
        /// Original condition builder error.
        source: ConditionBuildError,
    },
    /// A transition failed structural validation.
    #[error("transition {id:?}: {source}")]
    Transition {
        /// Invalid transition identity.
        id: I::Transition,
        /// Original transition builder error.
        source: TransitionBuildError,
    },
    /// A disposition failed structural validation.
    #[error("disposition {id:?}: {source}")]
    Disposition {
        /// Invalid disposition identity.
        id: I::Disposition,
        /// Original disposition builder error.
        source: DispositionBuildError,
    },
}

/// Result from checked neutral-record construction.
pub type RecordBuildResult<T, I = DefaultIds> = Result<T, RecordBuildError<I>>;
