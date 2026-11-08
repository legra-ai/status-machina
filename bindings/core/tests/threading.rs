//! Thread capability follows the interpreter host, never a blanket unsafe
//! claim.
use std::sync::atomic::{
    AtomicU32,
    Ordering,
};

use status_machina::EvaluationError;
use status_machina_bridge::callback::{
    Request,
    Response,
};
use status_machina_bridge::value::{
    ArenaState,
    Host,
};

#[derive(Debug)]
struct ThreadSafeHost {
    arena: ArenaState,
    next: AtomicU32,
}
impl Host for ThreadSafeHost {
    fn arena(&self) -> &ArenaState {
        &self.arena
    }
    fn execute(&self, _: Request) -> Result<Response, EvaluationError> {
        Ok(Response::Value(self.next.fetch_add(1, Ordering::SeqCst)))
    }
}
fn requires_send<T: Send>() {}
fn requires_sync<T: Sync>() {}
#[test]
fn python_capable_host_keeps_native_instances_sendable_and_definitions_shareable() {
    requires_send::<status_machina_bridge::runtime::Machine<ThreadSafeHost>>();
    requires_sync::<status_machina_bridge::build::Template<ThreadSafeHost>>();
}
