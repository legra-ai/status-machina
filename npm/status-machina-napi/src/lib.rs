//! Private N-API adapter. Application values remain in JavaScript-owned arenas.
mod error;
mod host;

use std::cell::{
    Ref,
    RefCell,
    RefMut,
};
use std::sync::Arc;
use std::thread::{
    self,
    ThreadId,
};

use error::{
    CoreError,
    encode,
    translate,
};
use host::JsHost;
use napi::Env;
use napi::bindgen_prelude::Function;
use napi_derive::napi;
use status_machina_bridge::build::Template;
use status_machina_bridge::codec::{
    Codec,
    CodecFailure,
};
use status_machina_bridge::runtime::{
    Decision,
    Machine,
};
use status_machina_bridge::value::Host;

fn read<T>(value: &RefCell<T>) -> napi::Result<Ref<'_, T>> {
    value.try_borrow().map_err(|_| translate(CoreError::Busy))
}
fn write<T>(value: &RefCell<T>) -> napi::Result<RefMut<'_, T>> {
    value
        .try_borrow_mut()
        .map_err(|_| translate(CoreError::Busy))
}
fn revision(value: Option<String>) -> napi::Result<Option<u64>> {
    value
        .map(|value| {
            value
                .parse()
                .map_err(|e| translate(CoreError::construction("revision", e)))
        })
        .transpose()
}
#[napi]
pub struct NativeDefinition {
    template: Arc<Template<JsHost>>,
    environment: usize,
    thread: ThreadId,
}
#[napi]
impl NativeDefinition {
    #[napi(constructor)]
    pub fn new(env: Env, source: String) -> napi::Result<Self> {
        Ok(Self {
            template: Arc::new(Template::parse(&source).map_err(translate)?),
            environment: env.raw() as usize,
            thread: thread::current().id(),
        })
    }
    /// Bind a reference-only portable record through `DefinitionRecord::bind`.
    #[napi(factory)]
    pub fn bind_record(env: Env, source: String) -> napi::Result<Self> {
        Ok(Self {
            template: Arc::new(Template::bind_record(&source).map_err(translate)?),
            environment: env.raw() as usize,
            thread: thread::current().id(),
        })
    }
    #[napi]
    pub fn label_inventory(&self, env: Env) -> napi::Result<String> {
        self.check(env)?;
        Ok(encode(&self.template.labels))
    }
    #[napi]
    pub fn structural_coverage(&self, env: Env, entries: u32, bytes: u32) -> napi::Result<String> {
        self.check(env)?;
        Ok(encode(
            &self
                .template
                .structural_coverage(entries as usize, bytes as usize)
                .map_err(translate)?,
        ))
    }
    #[napi]
    pub fn diagram(
        &self,
        env: Env,
        labels: String,
        entries: u32,
        bytes: u32,
    ) -> napi::Result<String> {
        self.check(env)?;
        self.template
            .diagram(&labels, entries as usize, bytes as usize)
            .map_err(translate)
    }
    #[napi]
    #[allow(
        clippy::arc_with_non_send_sync,
        reason = "shared lease ownership uses Arc; Node callback hosts deliberately remain thread-bound"
    )]
    pub fn instantiate(
        &self,
        env: Env,
        initial: u32,
        capacity: u32,
        invoker: Function<'_, String, String>,
    ) -> napi::Result<NativeInstance> {
        self.check(env)?;
        let host = Arc::new(
            JsHost::try_new(env, capacity as usize)
                .map_err(|e| translate(CoreError::evaluation(e)))?,
        );
        let _frame = host.activate(env, invoker)?;
        let machine =
            Machine::try_new(&self.template, Arc::clone(&host), initial).map_err(translate)?;
        Ok(NativeInstance {
            machine: RefCell::new(machine),
            host,
        })
    }
}
impl NativeDefinition {
    fn check(&self, env: Env) -> napi::Result<()> {
        if self.environment != env.raw() as usize || self.thread != thread::current().id() {
            return Err(translate(CoreError::construction(
                "environment",
                "definition belongs to another Node environment",
            )));
        }
        Ok(())
    }
}
#[napi]
pub struct NativeInstance {
    machine: RefCell<Machine<JsHost>>,
    host: Arc<JsHost>,
}
#[napi]
impl NativeInstance {
    #[napi]
    pub fn arena_id(&self, env: Env) -> napi::Result<String> {
        self.host.check(env)?;
        Ok(self.host.arena().id())
    }
    #[napi]
    pub fn live_keys(&self, env: Env) -> napi::Result<Vec<u32>> {
        self.host.check(env)?;
        Ok(self.host.arena().live())
    }
    #[napi]
    pub fn state(&self, env: Env) -> napi::Result<u32> {
        self.host.check(env)?;
        read(&self.machine)?.state().map_err(translate)
    }
    #[napi]
    pub fn revision(&self, env: Env) -> napi::Result<String> {
        self.host.check(env)?;
        Ok(read(&self.machine)?
            .revision()
            .map_err(translate)?
            .to_string())
    }
    #[napi]
    pub fn snapshot(
        &self,
        env: Env,
        invoker: Function<'_, String, String>,
    ) -> napi::Result<String> {
        let _frame = self.host.activate(env, invoker)?;
        Ok(encode(&read(&self.machine)?.snapshot().map_err(translate)?))
    }
    #[napi]
    pub fn dispatch(
        &self,
        env: Env,
        kind: u32,
        payload: u32,
        facts: u32,
        invoker: Function<'_, String, String>,
        expected_revision: Option<String>,
    ) -> napi::Result<String> {
        let _frame = self.host.activate(env, invoker)?;
        Ok(encode(
            &write(&self.machine)?
                .dispatch(kind, payload, facts, revision(expected_revision)?)
                .map_err(translate)?,
        ))
    }
    #[napi]
    pub fn evaluate(
        &self,
        env: Env,
        kind: u32,
        payload: u32,
        facts: u32,
        invoker: Function<'_, String, String>,
        expected_revision: Option<String>,
    ) -> napi::Result<NativeDecision> {
        let _frame = self.host.activate(env, invoker)?;
        Ok(NativeDecision {
            decision: RefCell::new(Some(
                read(&self.machine)?
                    .evaluate(kind, payload, facts, revision(expected_revision)?)
                    .map_err(translate)?,
            )),
        })
    }
    #[napi]
    pub fn evaluate_capability(
        &self,
        env: Env,
        id: u32,
        facts: u32,
        invoker: Function<'_, String, String>,
    ) -> napi::Result<String> {
        let _frame = self.host.activate(env, invoker)?;
        Ok(encode(
            &read(&self.machine)?
                .evaluate_capability(id, facts)
                .map_err(translate)?
                .report,
        ))
    }
    #[napi]
    pub fn accept(&self, env: Env, decision: &NativeDecision) -> napi::Result<String> {
        self.host.check(env)?;
        let mut machine = write(&self.machine)?;
        let decision = write(&decision.decision)?
            .take()
            .ok_or_else(|| translate(CoreError::Disposed))?;
        Ok(encode(&machine.accept(decision).map_err(translate)?))
    }
    #[napi]
    pub fn dispose(&self, env: Env) -> napi::Result<()> {
        self.host.check(env)?;
        write(&self.machine)?.dispose();
        Ok(())
    }
}
#[napi]
pub struct NativeDecision {
    decision: RefCell<Option<Decision<JsHost>>>,
}
#[napi]
impl NativeDecision {
    #[napi]
    pub fn expected_revision(&self) -> napi::Result<String> {
        Ok(read(&self.decision)?
            .as_ref()
            .ok_or_else(|| translate(CoreError::Disposed))?
            .expected_revision()
            .to_string())
    }
    #[napi]
    pub fn dispose(&self) -> napi::Result<()> {
        write(&self.decision)?.take();
        Ok(())
    }
}
/// Kernel `IdentityMap` over private per-codec indexes; values stay in
/// JavaScript.
#[napi]
pub struct NativeIdentityMap {
    codec: Codec,
}
#[napi]
impl NativeIdentityMap {
    /// Pair `identities[i]` with `representations[i]`, in caller order.
    #[napi(constructor)]
    pub fn new(
        identities: Vec<u32>,
        representations: Vec<u32>,
        max_entries: u32,
    ) -> napi::Result<Self> {
        if identities.len() != representations.len() {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                "identity and representation indexes must pair up",
            ));
        }
        // bounded: JavaScript consumes at most max_entries + 1 entries, and the
        // kernel ceiling is 4096.
        let pairs: Vec<(u32, u32)> = identities.into_iter().zip(representations).collect();
        Ok(Self {
            codec: Codec::try_new(&pairs, max_entries as usize).map_err(codec_failure)?,
        })
    }
    #[napi]
    pub fn encode(&self, identity: u32) -> napi::Result<u32> {
        self.codec.encode(identity).map_err(codec_failure)
    }
    #[napi]
    pub fn decode(&self, representation: u32) -> napi::Result<u32> {
        self.codec.decode(representation).map_err(codec_failure)
    }
}
fn codec_failure(failure: CodecFailure) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, failure.encode())
}
