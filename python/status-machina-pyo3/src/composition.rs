//! Kernel composition over exclusively locked native instances; values stay in
//! visible Python arenas.
use std::sync::{
    Arc,
    MutexGuard,
};

use pyo3::prelude::*;
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
    NativeFailure,
    encode,
    translate,
};
use crate::host::PyHost;
use crate::{
    NativeInstance,
    lock,
};

type KernelReport = status_machina::CapabilityReport<status_machina_bridge::ids::Ids<PyHost>>;

fn fault(py: Python<'_>, fault: Fault<PyHost>) -> PyErr {
    match fault {
        Fault::Composition(failure) => NativeFailure::new_err(failure.encode()),
        Fault::Core(error) => translate(py, error),
    }
}
/// Lock each distinct instance; Rust exclusive borrows cannot alias one.
fn exclusive<'a>(
    py: Python<'_>,
    machines: &'a [Bound<'_, NativeInstance>],
) -> PyResult<Vec<MutexGuard<'a, Machine<PyHost>>>> {
    for (entry, machine) in machines.iter().enumerate() {
        if machines[..entry].iter().any(|other| other.is(machine)) {
            return Err(NativeFailure::new_err(Failure::aliased(entry).encode()));
        }
    }
    machines
        .iter()
        .map(|machine| lock(py, &machine.get().machine))
        .collect()
}
/// A call-local value arena for caller inputs and facts; no callback runs in
/// it.
#[pyclass(name = "_Arena", frozen, module = "status_machina._native")]
pub(crate) struct NativeArena {
    host: Arc<PyHost>,
}
#[pymethods]
impl NativeArena {
    #[new]
    fn new(py: Python<'_>, capacity: usize) -> PyResult<Self> {
        Ok(Self {
            host: Arc::new(
                PyHost::try_new(capacity).map_err(|e| translate(py, CoreError::evaluation(e)))?,
            ),
        })
    }
    fn arena_id(&self) -> String {
        self.host.arena().id()
    }
}
/// The kernel report behind one `Machine.evaluate_capability` observation.
#[pyclass(name = "_CapabilityReport", frozen, module = "status_machina._native")]
pub(crate) struct NativeCapabilityReport {
    pub(crate) report: KernelReport,
}
#[pyclass(
    name = "_CapabilitySnapshot",
    frozen,
    module = "status_machina._native"
)]
pub(crate) struct NativeSnapshot {
    snapshot: Snapshot<PyHost>,
}
#[pyclass(name = "_Composition", frozen, module = "status_machina._native")]
pub(crate) struct NativeComposition {
    topology: Topology<PyHost>,
}
#[pymethods]
impl NativeComposition {
    #[new]
    fn new(
        py: Python<'_>,
        source: &str,
        machines: Vec<Bound<'_, NativeInstance>>,
    ) -> PyResult<Self> {
        let guards = exclusive(py, &machines)?;
        let machines: Vec<&Machine<PyHost>> = guards.iter().map(|guard| &**guard).collect();
        Ok(Self {
            topology: Topology::try_new(source, &machines).map_err(|e| fault(py, e))?,
        })
    }
    #[pyo3(signature = (arena, source, snapshot=None))]
    fn route_batch(
        &self,
        py: Python<'_>,
        arena: &NativeArena,
        source: &str,
        snapshot: Option<PyRef<'_, NativeSnapshot>>,
    ) -> PyResult<String> {
        let snapshot = snapshot.as_ref().map(|value| &value.snapshot);
        Ok(encode(
            &self
                .topology
                .route_batch(&arena.host, source, snapshot)
                .map_err(|e| fault(py, e))?,
        ))
    }
    fn snapshot(
        &self,
        py: Python<'_>,
        source: &str,
        reports: Vec<PyRef<'_, NativeCapabilityReport>>,
    ) -> PyResult<NativeSnapshot> {
        let reports: Vec<&KernelReport> = reports.iter().map(|value| &value.report).collect();
        Ok(NativeSnapshot {
            snapshot: self
                .topology
                .snapshot(source, &reports)
                .map_err(|e| fault(py, e))?,
        })
    }
    fn bind(
        &self,
        py: Python<'_>,
        source: &str,
        machines: Vec<Bound<'_, NativeInstance>>,
    ) -> PyResult<()> {
        let mut guards = exclusive(py, &machines)?;
        let mut machines: Vec<&mut Machine<PyHost>> =
            guards.iter_mut().map(|guard| &mut **guard).collect();
        self.topology
            .bind(&mut machines, source)
            .map_err(|e| fault(py, e))
    }
    /// Return the addressed report and, when dispatch failed, the translated
    /// failure carrying the original Python exception as its cause.
    #[pyo3(signature = (arena, source, machines, invokers, snapshot=None))]
    fn propagate(
        &self,
        py: Python<'_>,
        arena: &NativeArena,
        source: &str,
        machines: Vec<Bound<'_, NativeInstance>>,
        invokers: Vec<Py<PyAny>>,
        snapshot: Option<PyRef<'_, NativeSnapshot>>,
    ) -> PyResult<(String, Option<Py<PyAny>>)> {
        if invokers.len() != machines.len() {
            return Err(translate(
                py,
                CoreError::construction("binding", "invokers must pair with machines"),
            ));
        }
        let mut guards = exclusive(py, &machines)?;
        // bounded: one activation per exclusively locked machine.
        let mut frames = Vec::with_capacity(machines.len());
        for (machine, invoker) in machines.iter().zip(invokers) {
            frames.push(
                machine
                    .get()
                    .host
                    .activate(invoker)
                    .map_err(|e| translate(py, e))?,
            );
        }
        let mut bound: Vec<&mut Machine<PyHost>> =
            guards.iter_mut().map(|guard| &mut **guard).collect();
        let snapshot = snapshot.as_ref().map(|value| &value.snapshot);
        let propagated = self
            .topology
            .propagate(&mut bound, &arena.host, source, snapshot)
            .map_err(|e| fault(py, e))?;
        drop(frames);
        let failure = propagated
            .failure
            .map(|error| translate(py, error).into_value(py).into_any());
        Ok((encode(&propagated.report), failure))
    }
}
/// Validate explicit propagation budgets with the kernel builder.
#[pyfunction]
#[pyo3(signature = (steps, intents))]
pub(crate) fn propagation_limits(steps: Option<usize>, intents: Option<usize>) -> PyResult<()> {
    limits(steps, intents)
        .map(drop)
        .map_err(|failure| NativeFailure::new_err(failure.encode()))
}
