//! Private identity family for checking scenario-set bounds by count alone.
use status_machina::MachineIds;

/// The kernel's set bounds depend only on how many scenarios and steps are
/// supplied, so a construction check carries no identities or payloads.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CountIds;
impl MachineIds for CountIds {
    type Definition = ();
    type Instance = ();
    type State = ();
    type Event = ();
    type Transition = ();
    type Action = ();
    type Condition = ();
    type Effect = ();
    type Disposition = ();
    type Capability = ();
    type Operation = ();
    type Scenario = ();
    type Assertion = ();
    type Scope = ();
    type Behavior = ();
    type Implementation = ();
}
