//! Kernel composition over exclusively borrowed native instances; values stay
//! in JavaScript-owned arenas.
use std::cell::RefMut;
use std::sync::Arc;

use napi::Env;
use napi::bindgen_prelude::Function;
use napi_derive::napi;
use status_machina_bridge::composition::{
    Failure,
    Fault,
    Snapshot,
    Topology,
    limits,
};
use status_machina_bridge::runtime::Machine;
use status_machina_bridge::value::Host;

use crate::error::{
    CoreError,
    encode,
    translate,
};
use crate::host::JsHost;
use crate::{
    NativeInstance,
    write,
};

type KernelReport = status_machina::CapabilityReport<status_machina_bridge::ids::Ids<JsHost>>;

fn failure(failure: &Failure) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, failure.encode())
}
fn fault(fault: Fault<JsHost>) -> napi::Error {
    match fault {
        Fault::Composition(value) => failure(&value),
        Fault::Core(error) => translate(error),
    }
}
/// Borrow each distinct instance mutably; Rust exclusive borrows cannot alias.
fn exclusive<'a>(
    env: Env,
    machines: &[&'a NativeInstance],
) -> napi::Result<Vec<RefMut<'a, Machine<JsHost>>>> {
    for (entry, machine) in machines.iter().enumerate() {
        machine.host.check(env)?;
        if machines[..entry]
            .iter()
            .any(|other| std::ptr::eq(*other, *machine))
        {
            return Err(failure(&Failure::aliased(entry)));
        }
    }
    machines
        .iter()
        .map(|machine| write(&machine.machine))
        .collect()
}
/// A call-local value arena for caller inputs and facts; no callback runs in
/// it.
#[napi]
pub struct NativeArena {
    host: Arc<JsHost>,
}
#[napi]
impl NativeArena {
    #[napi(constructor)]
    #[allow(
        clippy::arc_with_non_send_sync,
        reason = "shared lease ownership uses Arc; Node hosts deliberately remain thread-bound"
    )]
    pub fn new(env: Env, capacity: u32) -> napi::Result<Self> {
        Ok(Self {
            host: Arc::new(
                JsHost::try_new(env, capacity as usize)
                    .map_err(|e| translate(CoreError::evaluation(e)))?,
            ),
        })
    }
    #[napi]
    pub fn arena_id(&self) -> String {
        self.host.arena().id()
    }
}
/// The kernel report behind one `Machine.evaluateCapability` observation.
#[napi]
pub struct NativeCapabilityReport {
    pub(crate) report: KernelReport,
}
#[napi]
pub struct NativeSnapshot {
    snapshot: Snapshot<JsHost>,
}
/// The addressed report and, when dispatch failed, the translated failure
/// carrying the original JavaScript exception as its cause.
#[napi(object, object_from_js = false)]
pub struct NativePropagation {
    pub report: String,
    pub failure: Option<napi::Error>,
}
#[napi]
pub struct NativeComposition {
    topology: Topology<JsHost>,
}
#[napi]
impl NativeComposition {
    #[napi(constructor)]
    pub fn new(env: Env, source: String, machines: Vec<&NativeInstance>) -> napi::Result<Self> {
        let guards = exclusive(env, &machines)?;
        let machines: Vec<&Machine<JsHost>> = guards.iter().map(|guard| &**guard).collect();
        Ok(Self {
            topology: Topology::try_new(&source, &machines).map_err(fault)?,
        })
    }
    #[napi]
    pub fn route_batch(
        &self,
        arena: &NativeArena,
        source: String,
        snapshot: Option<&NativeSnapshot>,
    ) -> napi::Result<String> {
        Ok(encode(
            &self
                .topology
                .route_batch(&arena.host, &source, snapshot.map(|value| &value.snapshot))
                .map_err(fault)?,
        ))
    }
    #[napi]
    pub fn snapshot(
        &self,
        source: String,
        reports: Vec<&NativeCapabilityReport>,
    ) -> napi::Result<NativeSnapshot> {
        let reports: Vec<&KernelReport> = reports.iter().map(|value| &value.report).collect();
        Ok(NativeSnapshot {
            snapshot: self.topology.snapshot(&source, &reports).map_err(fault)?,
        })
    }
    #[napi]
    pub fn bind(
        &self,
        env: Env,
        source: String,
        machines: Vec<&NativeInstance>,
    ) -> napi::Result<()> {
        let mut guards = exclusive(env, &machines)?;
        let mut machines: Vec<&mut Machine<JsHost>> =
            guards.iter_mut().map(|guard| &mut **guard).collect();
        self.topology.bind(&mut machines, &source).map_err(fault)
    }
    #[napi]
    pub fn propagate(
        &self,
        env: Env,
        arena: &NativeArena,
        source: String,
        machines: Vec<&NativeInstance>,
        invokers: Vec<Function<'_, String, String>>,
        snapshot: Option<&NativeSnapshot>,
    ) -> napi::Result<NativePropagation> {
        if invokers.len() != machines.len() {
            return Err(translate(CoreError::construction(
                "binding",
                "invokers must pair with machines",
            )));
        }
        arena.host.check(env)?;
        let mut guards = exclusive(env, &machines)?;
        // bounded: one activation per exclusively borrowed machine.
        let mut frames = Vec::with_capacity(machines.len());
        for (machine, invoker) in machines.iter().zip(invokers) {
            frames.push(machine.host.activate(env, invoker)?);
        }
        let mut bound: Vec<&mut Machine<JsHost>> =
            guards.iter_mut().map(|guard| &mut **guard).collect();
        let propagated = self
            .topology
            .propagate(
                &mut bound,
                &arena.host,
                &source,
                snapshot.map(|value| &value.snapshot),
            )
            .map_err(fault)?;
        drop(frames);
        Ok(NativePropagation {
            report: encode(&propagated.report),
            failure: propagated.failure.map(translate),
        })
    }
}
/// Validate explicit propagation budgets with the kernel builder.
#[napi]
pub fn propagation_limits(steps: Option<u32>, intents: Option<u32>) -> napi::Result<()> {
    limits(steps.map(|v| v as usize), intents.map(|v| v as usize))
        .map(drop)
        .map_err(|value| failure(&value))
}
