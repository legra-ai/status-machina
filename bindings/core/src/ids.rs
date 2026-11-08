//! Private engine IDs never replace native application identifier values.
use std::fmt;
use std::hash::{
    Hash,
    Hasher,
};
use std::marker::PhantomData;
use std::sync::Arc;

use crate::value::Host;

pub struct Ids<H: Host>(PhantomData<fn() -> H>);
impl<H: Host> Clone for Ids<H> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<H: Host> Copy for Ids<H> {}
impl<H: Host> fmt::Debug for Ids<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeIds")
    }
}
impl<H: Host> PartialEq for Ids<H> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl<H: Host> Eq for Ids<H> {}
impl<H: Host> Hash for Ids<H> {
    fn hash<S: Hasher>(&self, _: &mut S) {}
}
impl<H: Host> status_machina::MachineIds for Ids<H> {
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
    type Scenario = ScenarioKey<H>;
    type Assertion = u32;
    type Scope = u32;
    type Behavior = u32;
    type Implementation = u32;
}
pub struct ScenarioKey<H: Host> {
    pub index: u32,
    pub host: Arc<H>,
}
impl<H: Host> Clone for ScenarioKey<H> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            host: Arc::clone(&self.host),
        }
    }
}
impl<H: Host> fmt::Debug for ScenarioKey<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.index.fmt(f)
    }
}
impl<H: Host> PartialEq for ScenarioKey<H> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.host.arena().id() == other.host.arena().id()
    }
}
impl<H: Host> Eq for ScenarioKey<H> {}
impl<H: Host> Hash for ScenarioKey<H> {
    fn hash<S: Hasher>(&self, state: &mut S) {
        self.index.hash(state);
        self.host.arena().id().hash(state);
    }
}
