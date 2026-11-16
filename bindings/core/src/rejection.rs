//! Kernel construction rejections as typed native categories.
//!
//! `code` selects the native exception class and `reason` names the kernel
//! variant, so no construction error reaches a language as text alone.
use status_machina::{
    ActionBuildError,
    BindingError,
    CapabilityBuildError,
    ConditionBuildError,
    DefinitionBuildError,
    DispositionBuildError,
    EffectBuildError,
    OutputEventBuildError,
    TransitionBuildError,
};

/// A kernel rejection with a stable native category and variant name.
pub trait Rejection: std::fmt::Display {
    /// Native exception category.
    fn code(&self) -> &'static str;
    /// Kernel variant, in snake case.
    fn reason(&self) -> &'static str;
}
impl Rejection for ConditionBuildError {
    fn code(&self) -> &'static str {
        match self {
            Self::OperandLimitExceeded | Self::DepthLimitExceeded => "definition_limit",
            _ => "invalid_condition",
        }
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::MissingPredicate => "missing_predicate",
            Self::EmptyGroup => "empty_group",
            Self::OperandLimitExceeded => "operand_limit_exceeded",
            Self::DepthLimitExceeded => "depth_limit_exceeded",
            Self::InvalidGroupKind => "invalid_group_kind",
            Self::EmptyNegation => "empty_negation",
            Self::DuplicateOperand => "duplicate_operand",
        }
    }
}
impl Rejection for TransitionBuildError {
    fn code(&self) -> &'static str {
        match self {
            Self::InternalTransitionMustBeSelf => "internal_transition",
            Self::EffectLimitExceeded
            | Self::ActionLimitExceeded
            | Self::PostconditionLimitExceeded
            | Self::OutputEventLimitExceeded => "definition_limit",
            Self::MissingSource | Self::MissingEvent | Self::MissingDestination => "definition",
        }
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::MissingSource => "missing_source",
            Self::MissingEvent => "missing_event",
            Self::MissingDestination => "missing_destination",
            Self::EffectLimitExceeded => "effect_limit_exceeded",
            Self::ActionLimitExceeded => "action_limit_exceeded",
            Self::InternalTransitionMustBeSelf => "internal_transition_must_be_self",
            Self::PostconditionLimitExceeded => "postcondition_limit_exceeded",
            Self::OutputEventLimitExceeded => "output_event_limit_exceeded",
        }
    }
}
impl Rejection for DispositionBuildError {
    fn code(&self) -> &'static str {
        "invalid_disposition"
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::MissingState => "missing_state",
            Self::MissingEvent => "missing_event",
            Self::MissingCondition => "missing_condition",
            Self::MissingReason => "missing_reason",
            Self::InvalidReason => "invalid_reason",
        }
    }
}
impl Rejection for ActionBuildError {
    fn code(&self) -> &'static str {
        "definition"
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::MissingImplementation => "missing_implementation",
        }
    }
}
impl Rejection for EffectBuildError {
    fn code(&self) -> &'static str {
        "definition"
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::MissingConstructor => "missing_constructor",
        }
    }
}
impl Rejection for OutputEventBuildError {
    fn code(&self) -> &'static str {
        "definition"
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::MissingConstructor => "missing_constructor",
        }
    }
}
impl Rejection for CapabilityBuildError {
    fn code(&self) -> &'static str {
        "definition"
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::MissingEvaluator => "missing_evaluator",
        }
    }
}
impl<I: status_machina::MachineIds> Rejection for BindingError<I> {
    fn code(&self) -> &'static str {
        match self {
            Self::UnknownBehavior { .. } => "unknown_behavior",
            Self::UnknownImplementation { .. } => "unknown_implementation",
            Self::SignatureMismatch { .. } => "role_mismatch",
            Self::DuplicateImplementation { .. } => "duplicate_implementation",
            Self::InvalidLimit | Self::LimitBelowCurrentSize | Self::LimitExceeded { .. } => {
                "definition_limit"
            }
        }
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::UnknownBehavior { .. } => "unknown_behavior",
            Self::UnknownImplementation { .. } => "unknown_implementation",
            Self::SignatureMismatch { .. } => "signature_mismatch",
            Self::DuplicateImplementation { .. } => "duplicate_implementation",
            Self::InvalidLimit => "invalid_limit",
            Self::LimitBelowCurrentSize => "limit_below_current_size",
            Self::LimitExceeded { .. } => "limit_exceeded",
        }
    }
}
impl<I: status_machina::MachineIds> Rejection for DefinitionBuildError<I> {
    fn code(&self) -> &'static str {
        match self {
            Self::EvaluationFailed { .. } => "callback",
            Self::UnknownInitialState { .. }
            | Self::UnknownTransitionState { .. }
            | Self::UnknownDispositionState { .. } => "unknown_state",
            Self::DuplicateState { .. }
            | Self::DuplicateTransition { .. }
            | Self::DuplicateDisposition { .. } => "duplicate_identifier",
            Self::UnreachableState { .. } => "unreachable_state",
            Self::TerminalHasOutgoingTransition { .. } => "terminal_transition",
            Self::InitialInvariantFailed { .. } => "initial_invariant",
            Self::StateEffectLimitExceeded { .. }
            | Self::StateActionLimitExceeded { .. }
            | Self::EmptyLimits
            | Self::LimitsBelowCurrentSize
            | Self::StateLimitExceeded { .. }
            | Self::TransitionLimitExceeded { .. }
            | Self::DispositionLimitExceeded { .. } => "definition_limit",
            Self::MissingInitialState => "definition",
        }
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::EvaluationFailed { .. } => "evaluation_failed",
            Self::StateEffectLimitExceeded { .. } => "state_effect_limit_exceeded",
            Self::MissingInitialState => "missing_initial_state",
            Self::UnknownInitialState { .. } => "unknown_initial_state",
            Self::DuplicateState { .. } => "duplicate_state",
            Self::DuplicateTransition { .. } => "duplicate_transition",
            Self::DuplicateDisposition { .. } => "duplicate_disposition",
            Self::UnknownTransitionState { .. } => "unknown_transition_state",
            Self::UnknownDispositionState { .. } => "unknown_disposition_state",
            Self::UnreachableState { .. } => "unreachable_state",
            Self::EmptyLimits => "empty_limits",
            Self::LimitsBelowCurrentSize => "limits_below_current_size",
            Self::StateLimitExceeded { .. } => "state_limit_exceeded",
            Self::TransitionLimitExceeded { .. } => "transition_limit_exceeded",
            Self::DispositionLimitExceeded { .. } => "disposition_limit_exceeded",
            Self::TerminalHasOutgoingTransition { .. } => "terminal_has_outgoing_transition",
            Self::StateActionLimitExceeded { .. } => "state_action_limit_exceeded",
            Self::InitialInvariantFailed { .. } => "initial_invariant_failed",
        }
    }
}
