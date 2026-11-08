//! Native object leases. No application payload is serialized or stored in
//! Rust.
use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;

use serde::{
    Deserialize,
    Serialize,
};
use status_machina::{
    EvaluationError,
    FallibleCandidateData,
};

use crate::callback::{
    CallbackKind,
    Request,
    Response,
};

/// One private reference into a language-owned, GC-visible object arena.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValueKey {
    pub arena: String,
    pub key: u32,
}

/// The actual interpreter adapter, invoked only during an explicit native call.
pub trait Host {
    fn arena(&self) -> &ArenaState;
    fn execute(&self, request: Request) -> Result<Response, EvaluationError>;
}
/// Bounded ownership inventory; the language sweeps unleased objects after
/// calls.
pub struct ArenaState {
    id: uuid::Uuid,
    // bounded: live values cannot exceed the validated per-instance capacity.
    live: RefCell<HashSet<u32>>,
    limit: usize,
}
impl ArenaState {
    pub fn new(limit: usize) -> Result<Self, EvaluationError> {
        if limit == 0 || limit > 65_536 {
            return Err(EvaluationError::new(
                "native value capacity must be 1..=65536",
            ));
        }
        Ok(Self {
            id: uuid::Uuid::now_v7(),
            live: RefCell::new(HashSet::new()),
            limit,
        })
    }
    pub fn id(&self) -> String {
        self.id.to_string()
    }
    pub fn live(&self) -> Vec<u32> {
        // bounded: inventory size is capped by limit.
        self.live.borrow().iter().copied().collect()
    }
    fn claim(&self, key: u32) -> Result<(), EvaluationError> {
        let mut live = self.live.borrow_mut();
        if live.len() == self.limit {
            return Err(EvaluationError::new("native value capacity exhausted"));
        }
        if !live.insert(key) {
            return Err(EvaluationError::new("native value key is already leased"));
        }
        Ok(())
    }
    fn release(&self, key: u32) {
        assert!(
            self.live.borrow_mut().remove(&key),
            "native lease must be live exactly once"
        );
    }
}
struct Lease {
    host: Rc<dyn Host>,
    key: u32,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.host.arena().release(self.key);
    }
}
/// A counted lease; cloning shares an immutable reference, not application
/// data.
#[derive(Clone)]
pub struct Value(Rc<Lease>);
impl Value {
    pub fn claim(host: Rc<dyn Host>, key: u32) -> Result<Self, EvaluationError> {
        host.arena().claim(key)?;
        Ok(Self(Rc::new(Lease { host, key })))
    }
    pub fn key(&self) -> ValueKey {
        ValueKey {
            arena: self.0.host.arena().id(),
            key: self.0.key,
        }
    }
    pub fn host(&self) -> &Rc<dyn Host> {
        &self.0.host
    }
}
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.key().fmt(f)
    }
}
/// Candidate data is a lease into an arena owned by the visible machine object.
pub struct Data {
    pub value: Value,
}
impl FallibleCandidateData for Data {
    fn try_snapshot(&self) -> Result<Self, EvaluationError> {
        let host = self.value.host();
        let request = Request::new(CallbackKind::Snapshot, None, vec![self.value.key()]);
        let Response::Value(key) = host.execute(request)? else {
            return Err(EvaluationError::new("snapshot must return a value key"));
        };
        Ok(Self {
            value: Value::claim(Rc::clone(host), key)?,
        })
    }
    fn prepare_accept(self) -> Result<Self, EvaluationError> {
        self.try_snapshot()
    }
}
