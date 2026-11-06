use super::{
    BehaviorRef,
    BehaviorRole,
};

/// Refused registration or resolution; no callback was invoked.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BindingError<I: crate::MachineIds = crate::DefaultIds> {
    /// The requested registry bound is zero or above 4096.
    #[error("binding limit must be between 1 and 4096")]
    InvalidLimit,
    /// A new limit is below the number of existing declarations.
    #[error("binding limit is below the registered count")]
    LimitBelowCurrentSize,
    /// Another unique declaration would exceed the registry bound.
    #[error("binding limit {limit} exceeded")]
    LimitExceeded {
        /// Configured total declaration limit, across all roles and versions.
        limit: usize,
    },
    /// The exact Story/implementation pair was already registered.
    #[error("duplicate behavior implementation {reference:?}")]
    DuplicateImplementation {
        /// Conflicting declaration.
        reference: BehaviorRef<I>,
    },
    /// No implementation of the requested Story is registered.
    #[error("unknown behavior {behavior:?}")]
    UnknownBehavior {
        /// Missing Story identity.
        behavior: I::Behavior,
    },
    /// The Story exists but the requested implementation does not.
    #[error("unknown behavior implementation {reference:?}")]
    UnknownImplementation {
        /// Missing implementation selection; no substitution is attempted.
        reference: BehaviorRef<I>,
    },
    /// The registered signature class does not match this use/declaration.
    #[error("behavior {reference:?} requires {expected:?}, found {actual:?}")]
    SignatureMismatch {
        /// Selected behavior/implementation.
        reference: BehaviorRef<I>,
        /// Signature required by the caller or existing Story declaration.
        expected: BehaviorRole,
        /// Signature supplied by the registration or selected implementation.
        actual: BehaviorRole,
    },
}
