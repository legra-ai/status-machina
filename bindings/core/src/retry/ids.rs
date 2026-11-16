//! Private retry identity family: the owning instance and interned operations.
use status_machina::MachineIds;

/// Whether a correlation names the instance that owns the wait set. Native
/// instance identities never enter Rust; each language compares them itself.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Owner {
    This,
    Other,
}

/// Retry waits identify operations by a private per-wait-set index; `None`
/// is an operation the wait set has never interned, so it matches no wait.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RetryIds;
impl MachineIds for RetryIds {
    type Definition = ();
    type Instance = Owner;
    type State = ();
    type Event = ();
    type Transition = ();
    type Action = ();
    type Condition = ();
    type Effect = ();
    type Disposition = ();
    type Capability = ();
    type Operation = Option<u32>;
    type Scenario = ();
    type Assertion = ();
    type Scope = ();
    type Behavior = ();
    type Implementation = ();
}
