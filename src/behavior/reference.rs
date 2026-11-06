/// Caller-assigned behavior identity for the convenience family.
///
/// Custom families can use enums or domain newtypes instead. This is not a
/// label.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BehaviorId(u128);
impl BehaviorId {
    /// Construct from a caller-owned stable value; no ID is generated.
    pub const fn new(value: u128) -> Self {
        Self(value)
    }
    /// Return the caller-owned value.
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Caller-assigned implementation identity for the convenience family.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImplementationId(u128);
impl ImplementationId {
    /// Construct from a caller-owned stable value; no ID is generated.
    pub const fn new(value: u128) -> Self {
        Self(value)
    }
    /// Return the caller-owned value.
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Typed reusable Story identity and selected implementation identity.
///
/// A graph action/condition/effect ID identifies a use site; this reference
/// identifies the reusable behavior bound there. Neither identity is display
/// text. Explicit adapters may map these values to EKGF Story and
/// `StoryImplementation` IRIs. Identity equality does not prove code
/// equivalence: trusted registrants must assign a new implementation ID when
/// behavior changes.
///
/// Story and implementation identity roles cannot be interchanged:
///
/// ```compile_fail
/// use state_machine::{BehaviorId, BehaviorRef, DefaultIds, ImplementationId};
/// let reference = BehaviorRef::<DefaultIds>::new(ImplementationId::new(1), BehaviorId::new(1));
/// ```
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BehaviorRef<I: crate::MachineIds = crate::DefaultIds> {
    behavior: I::Behavior,
    implementation: I::Implementation,
}
impl<I: crate::MachineIds> BehaviorRef<I> {
    /// Select one implementation of a reusable behavior.
    pub const fn new(behavior: I::Behavior, implementation: I::Implementation) -> Self {
        Self {
            behavior,
            implementation,
        }
    }
    /// Reusable behavior (Story) identity.
    pub const fn behavior(&self) -> &I::Behavior {
        &self.behavior
    }
    /// Selected implementation identity.
    pub const fn implementation(&self) -> &I::Implementation {
        &self.implementation
    }
}

/// Typed callback signature class required at a behavior use site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BehaviorRole {
    /// Candidate-data mutation returning a local result.
    Action,
    /// Predicate over data, event payload and facts.
    Condition,
    /// State invariant over local data.
    Invariant,
    /// External-work payload constructor.
    Effect,
    /// Output-event payload constructor.
    OutputEvent,
    /// Advisory policy over state, data and facts.
    Capability,
    /// Independent replay observation predicate.
    Assertion,
}
