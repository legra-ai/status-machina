/// One successfully accepted transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionRecord<I: crate::MachineIds = crate::DefaultIds> {
    pub(super) transition: I::Transition,
    pub(super) from: I::State,
    pub(super) to: I::State,
    pub(super) previous_revision: u64,
    pub(super) revision: u64,
}

impl<I: crate::MachineIds> TransitionRecord<I> {
    /// Return the transition identifier.
    #[must_use]
    pub const fn transition(&self) -> &I::Transition {
        &self.transition
    }

    /// Return the source state.
    #[must_use]
    pub const fn from(&self) -> &I::State {
        &self.from
    }

    /// Return the destination state.
    #[must_use]
    pub const fn to(&self) -> &I::State {
        &self.to
    }

    /// Return the revision before acceptance.
    #[must_use]
    pub const fn previous_revision(&self) -> u64 {
        self.previous_revision
    }

    /// Return the accepted revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}
