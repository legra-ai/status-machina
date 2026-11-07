use thiserror::Error;

/// Checked construction result retaining the caller's identity family.
pub type DefinitionResult<T, I = crate::DefaultIds> = Result<T, DefinitionBuildError<I>>;

/// Invalid structure or configured bounds in a machine definition.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DefinitionBuildError<I: crate::MachineIds = crate::DefaultIds> {
    /// An entry or exit effect list exceeded its per-state bound.
    #[error("state {state:?} effect phase exceeds limit 16")]
    StateEffectLimitExceeded {
        /// State containing the oversized effect phase.
        state: I::State,
    },
    /// No initial state was declared.
    #[error("machine initial state is missing")]
    MissingInitialState,
    /// The initial state does not exist in the definition.
    #[error("unknown initial state {state:?}")]
    UnknownInitialState {
        /// Identifier of the missing initial state.
        state: I::State,
    },
    /// Two states use the same identifier.
    #[error("duplicate state {state:?}")]
    DuplicateState {
        /// Identifier used by both state definitions.
        state: I::State,
    },
    /// Two transitions use the same identifier.
    #[error("duplicate transition {transition:?}")]
    DuplicateTransition {
        /// Identifier used by both transition definitions.
        transition: I::Transition,
    },
    /// Two dispositions use the same identifier.
    #[error("duplicate disposition {disposition:?}")]
    DuplicateDisposition {
        /// Identifier used by both disposition rules.
        disposition: I::Disposition,
    },
    /// A transition refers to a missing state.
    #[error("transition {transition:?} refers to unknown state {state:?}")]
    UnknownTransitionState {
        /// Transition containing the invalid reference.
        transition: I::Transition,
        /// Missing source or destination state.
        state: I::State,
    },
    /// A disposition refers to a missing state.
    #[error("disposition {disposition:?} refers to unknown state {state:?}")]
    UnknownDispositionState {
        /// Disposition containing the invalid reference.
        disposition: I::Disposition,
        /// Missing source state.
        state: I::State,
    },
    /// A state cannot be reached from the initial state.
    #[error("state {state:?} is unreachable from the initial state")]
    UnreachableState {
        /// State not reachable by any declared edge.
        state: I::State,
    },
    /// A definition limit must be positive.
    #[error("machine definition limits must be positive")]
    EmptyLimits,
    /// A proposed limit is below items already added.
    #[error("machine definition limits are below the current graph size")]
    LimitsBelowCurrentSize,
    /// The state count reached its configured bound.
    #[error("machine state count exceeds limit {limit}")]
    StateLimitExceeded {
        /// Configured maximum state count.
        limit: usize,
    },
    /// The transition count reached its configured bound.
    #[error("machine transition count exceeds limit {limit}")]
    TransitionLimitExceeded {
        /// Configured maximum transition count.
        limit: usize,
    },
    /// The disposition count reached its configured bound.
    #[error("machine disposition count exceeds limit {limit}")]
    DispositionLimitExceeded {
        /// Configured maximum disposition count.
        limit: usize,
    },
    /// A terminal state declared an outgoing edge.
    #[error("terminal state {state:?} has an outgoing transition")]
    TerminalHasOutgoingTransition {
        /// Terminal state with the invalid edge.
        state: I::State,
    },
    /// State entry or exit actions reached their configured limit.
    #[error("state {state:?} action count exceeds limit 16")]
    StateActionLimitExceeded {
        /// State whose actions exceeded the limit.
        state: I::State,
    },
    /// Initial data violates the initial-state invariant.
    #[error("initial data violates state invariant for {state:?}")]
    InitialInvariantFailed {
        /// State whose invariant rejected the initial data.
        state: I::State,
    },
}
