use crate::{
    DefaultIds,
    MachineIds,
};

/// Explicit labels for diagram inspection; labels never determine identity.
///
/// Different identities may have identical labels. Implementations must return
/// bounded labels without executing machine behavior or performing I/O.
pub trait DiagramLabels<I: MachineIds> {
    /// Label one state.
    fn state<'a>(&'a self, id: &'a I::State) -> &'a str;
    /// Label one transition.
    fn transition<'a>(&'a self, id: &'a I::Transition) -> &'a str;
    /// Label one input/output event kind.
    fn event<'a>(&'a self, id: &'a I::Event) -> &'a str;
    /// Label one guard or postcondition.
    fn condition<'a>(&'a self, id: &'a I::Condition) -> &'a str;
    /// Label one explicit disposition.
    fn disposition<'a>(&'a self, id: &'a I::Disposition) -> &'a str;
}

impl DiagramLabels<DefaultIds> for DefaultIds {
    fn state<'a>(&'a self, id: &'a crate::StateId) -> &'a str {
        id.as_str()
    }
    fn transition<'a>(&'a self, id: &'a crate::TransitionId) -> &'a str {
        id.as_str()
    }
    fn event<'a>(&'a self, id: &'a crate::EventId) -> &'a str {
        id.as_str()
    }
    fn condition<'a>(&'a self, id: &'a crate::ConditionId) -> &'a str {
        id.as_str()
    }
    fn disposition<'a>(&'a self, id: &'a crate::DispositionId) -> &'a str {
        id.as_str()
    }
}
