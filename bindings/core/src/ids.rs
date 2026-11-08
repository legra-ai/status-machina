//! Private engine IDs never replace the application's native identifier values.
use std::fmt;
use std::hash::{
    Hash,
    Hasher,
};
use std::rc::Rc;

use crate::value::Host;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ids;
impl status_machina::MachineIds for Ids {
    type Definition = uuid::Uuid;
    type Instance = uuid::Uuid;
    type State = u32;
    type Event = u32;
    type Transition = u32;
    type Action = u32;
    type Condition = u32;
    type Effect = u32;
    type Disposition = u32;
    type Capability = u32;
    type Operation = u32;
    type Scenario = ScenarioKey;
    type Assertion = u32;
    type Scope = u32;
    type Behavior = u32;
    type Implementation = u32;
}
#[derive(Clone)]
pub struct ScenarioKey {
    pub index: u32,
    pub host: Rc<dyn Host>,
}
impl fmt::Debug for ScenarioKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.index.fmt(f)
    }
}
impl PartialEq for ScenarioKey {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.host.arena().id() == other.host.arena().id()
    }
}
impl Eq for ScenarioKey {}
impl Hash for ScenarioKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.host.arena().id().hash(state);
    }
}
