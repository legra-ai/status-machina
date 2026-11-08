//! Native object leases. No application payload is serialized or stored in
//! Rust.
use std::collections::HashSet;
use std::fmt;
use std::sync::{
    Arc,
    Mutex,
};

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
pub trait Host: fmt::Debug + 'static {
    fn arena(&self) -> &ArenaState;
    fn execute(&self, request: Request) -> Result<Response, EvaluationError>;
}
/// Bounded ownership inventory; the language sweeps unleased objects after
/// calls.
#[derive(Debug)]
pub struct ArenaState {
    id: uuid::Uuid,
    // bounded: live values cannot exceed the validated per-instance capacity.
    live: Mutex<HashSet<u32>>,
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
            live: Mutex::new(HashSet::new()),
            limit,
        })
    }
    pub fn id(&self) -> String {
        self.id.to_string()
    }
    pub fn live(&self) -> Vec<u32> {
        // bounded: inventory size is capped by limit.
        self.live
            .lock()
            .expect("native arena lock is not poisoned")
            .iter()
            .copied()
            .collect()
    }
    fn claim(&self, key: u32) -> Result<(), EvaluationError> {
        let mut live = self.live.lock().expect("native arena lock is not poisoned");
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
            self.live
                .lock()
                .expect("native arena lock is not poisoned")
                .remove(&key),
            "native lease must be live exactly once"
        );
    }
}
struct Lease<H: Host> {
    host: Arc<H>,
    key: u32,
}
impl<H: Host> Drop for Lease<H> {
    fn drop(&mut self) {
        self.host.arena().release(self.key);
    }
}
/// A counted lease; cloning shares an immutable reference, not application
/// data.
pub struct Value<H: Host>(Arc<Lease<H>>);
impl<H: Host> Clone for Value<H> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<H: Host> Value<H> {
    pub fn claim(host: Arc<H>, key: u32) -> Result<Self, EvaluationError> {
        host.arena().claim(key)?;
        Ok(Self(Arc::new(Lease { host, key })))
    }
    pub fn key(&self) -> ValueKey {
        ValueKey {
            arena: self.0.host.arena().id(),
            key: self.0.key,
        }
    }
    pub fn host(&self) -> &Arc<H> {
        &self.0.host
    }
}
impl<H: Host> fmt::Debug for Value<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.key().fmt(f)
    }
}
/// Candidate data is a lease into an arena owned by the visible machine object.
pub struct Data<H: Host> {
    pub value: Value<H>,
}
impl<H: Host> FallibleCandidateData for Data<H> {
    fn try_snapshot(&self) -> Result<Self, EvaluationError> {
        let host = self.value.host();
        let request = Request::new(CallbackKind::Snapshot, None, vec![self.value.key()]);
        let Response::Value(key) = host.execute(request)? else {
            return Err(EvaluationError::new("snapshot must return a value key"));
        };
        Ok(Self {
            value: Value::claim(Arc::clone(host), key)?,
        })
    }
    fn prepare_accept(self) -> Result<Self, EvaluationError> {
        self.try_snapshot()
    }
}
