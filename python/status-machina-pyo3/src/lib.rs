//! Private Python extension; native application objects stay in visible Python
//! arenas.
mod composition;
mod error;
mod host;
mod retry;

use std::sync::{
    Arc,
    Mutex,
    MutexGuard,
    TryLockError,
};

use composition::{
    NativeArena,
    NativeCapabilityReport,
    NativeComposition,
    NativeSnapshot,
    propagation_limits,
};
use error::{
    CoreError,
    NativeFailure,
    encode,
    translate,
};
use host::PyHost;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use retry::{
    NativeRetryPolicy,
    NativeRetryWaits,
};
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

pub(crate) fn lock<'a, T>(py: Python<'_>, mutex: &'a Mutex<T>) -> PyResult<MutexGuard<'a, T>> {
    match mutex.try_lock() {
        Ok(guard) => Ok(guard),
        Err(TryLockError::WouldBlock) => Err(translate(py, CoreError::Busy)),
        Err(TryLockError::Poisoned(_)) => {
            Err(PyRuntimeError::new_err("native resource lock is poisoned"))
        }
    }
}
#[pyclass(name = "_Definition", frozen, module = "status_machina._native")]
struct NativeDefinition {
    template: Arc<Template<PyHost>>,
}
#[pymethods]
impl NativeDefinition {
    #[new]
    fn new(py: Python<'_>, source: &str) -> PyResult<Self> {
        Ok(Self {
            template: Arc::new(Template::parse(source).map_err(|e| translate(py, e))?),
        })
    }
    /// Bind a reference-only portable record through `DefinitionRecord::bind`.
    #[staticmethod]
    fn bind_record(py: Python<'_>, source: &str) -> PyResult<Self> {
        Ok(Self {
            template: Arc::new(Template::bind_record(source).map_err(|e| translate(py, e))?),
        })
    }
    fn label_inventory(&self) -> String {
        encode(&self.template.labels)
    }
    fn structural_coverage(
        &self,
        py: Python<'_>,
        entries: usize,
        bytes: usize,
    ) -> PyResult<String> {
        Ok(encode(
            &self
                .template
                .structural_coverage(entries, bytes)
                .map_err(|e| translate(py, e))?,
        ))
    }
    fn diagram(
        &self,
        py: Python<'_>,
        labels: &str,
        entries: usize,
        bytes: usize,
    ) -> PyResult<String> {
        self.template
            .diagram(labels, entries, bytes)
            .map_err(|e| translate(py, e))
    }
    fn instantiate(
        &self,
        py: Python<'_>,
        initial: u32,
        capacity: usize,
        invoker: Py<PyAny>,
    ) -> PyResult<NativeInstance> {
        let host = Arc::new(
            PyHost::try_new(capacity).map_err(|e| translate(py, CoreError::evaluation(e)))?,
        );
        let _frame = host.activate(invoker).map_err(|e| translate(py, e))?;
        let machine = Machine::try_new(&self.template, Arc::clone(&host), initial)
            .map_err(|e| translate(py, e))?;
        Ok(NativeInstance {
            machine: Mutex::new(machine),
            host,
        })
    }
}
#[pyclass(name = "_Instance", frozen, module = "status_machina._native")]
pub(crate) struct NativeInstance {
    machine: Mutex<Machine<PyHost>>,
    host: Arc<PyHost>,
}
#[pymethods]
impl NativeInstance {
    fn arena_id(&self) -> String {
        self.host.arena().id()
    }
    fn live_keys(&self) -> Vec<u32> {
        self.host.arena().live()
    }
    fn state(&self, py: Python<'_>) -> PyResult<u32> {
        lock(py, &self.machine)?
            .state()
            .map_err(|e| translate(py, e))
    }
    fn revision(&self, py: Python<'_>) -> PyResult<u64> {
        lock(py, &self.machine)?
            .revision()
            .map_err(|e| translate(py, e))
    }
    fn snapshot(&self, py: Python<'_>, invoker: Py<PyAny>) -> PyResult<String> {
        let machine = lock(py, &self.machine)?;
        let _frame = self.host.activate(invoker).map_err(|e| translate(py, e))?;
        Ok(encode(&machine.snapshot().map_err(|e| translate(py, e))?))
    }
    #[pyo3(signature = (kind, payload, facts, invoker, revision=None))]
    fn dispatch(
        &self,
        py: Python<'_>,
        kind: u32,
        payload: u32,
        facts: u32,
        invoker: Py<PyAny>,
        revision: Option<u64>,
    ) -> PyResult<String> {
        let mut machine = lock(py, &self.machine)?;
        let _frame = self.host.activate(invoker).map_err(|e| translate(py, e))?;
        Ok(encode(
            &machine
                .dispatch(kind, payload, facts, revision)
                .map_err(|e| translate(py, e))?,
        ))
    }
    #[pyo3(signature = (kind, payload, facts, invoker, revision=None))]
    fn evaluate(
        &self,
        py: Python<'_>,
        kind: u32,
        payload: u32,
        facts: u32,
        invoker: Py<PyAny>,
        revision: Option<u64>,
    ) -> PyResult<NativeDecision> {
        let machine = lock(py, &self.machine)?;
        let _frame = self.host.activate(invoker).map_err(|e| translate(py, e))?;
        Ok(NativeDecision {
            decision: Mutex::new(Some(
                machine
                    .evaluate(kind, payload, facts, revision)
                    .map_err(|e| translate(py, e))?,
            )),
        })
    }
    /// Return the observation metadata and the kernel report a capability
    /// snapshot needs.
    fn evaluate_capability(
        &self,
        py: Python<'_>,
        id: u32,
        facts: u32,
        invoker: Py<PyAny>,
    ) -> PyResult<(String, NativeCapabilityReport)> {
        let machine = lock(py, &self.machine)?;
        let _frame = self.host.activate(invoker).map_err(|e| translate(py, e))?;
        let observation = machine
            .evaluate_capability(id, facts)
            .map_err(|e| translate(py, e))?;
        Ok((
            encode(&observation.report),
            NativeCapabilityReport {
                report: observation.kernel,
            },
        ))
    }
    fn accept(&self, py: Python<'_>, decision: &NativeDecision) -> PyResult<String> {
        let mut machine = lock(py, &self.machine)?;
        let mut owned = lock(py, &decision.decision)?;
        let decision = owned
            .take()
            .ok_or_else(|| translate(py, CoreError::Disposed))?;
        Ok(encode(
            &machine.accept(decision).map_err(|e| translate(py, e))?,
        ))
    }
    fn dispose(&self, py: Python<'_>) -> PyResult<()> {
        lock(py, &self.machine)?.dispose();
        Ok(())
    }
}
#[pyclass(name = "_Decision", frozen, module = "status_machina._native")]
struct NativeDecision {
    decision: Mutex<Option<Decision<PyHost>>>,
}
#[pymethods]
impl NativeDecision {
    fn expected_revision(&self, py: Python<'_>) -> PyResult<u64> {
        Ok(lock(py, &self.decision)?
            .as_ref()
            .ok_or_else(|| translate(py, CoreError::Disposed))?
            .expected_revision())
    }
    fn dispose(&self, py: Python<'_>) -> PyResult<()> {
        lock(py, &self.decision)?.take();
        Ok(())
    }
}
/// Kernel `IdentityMap` over private per-codec indexes; values stay in Python.
#[pyclass(name = "_IdentityMap", frozen, module = "status_machina._native")]
struct NativeIdentityMap {
    codec: Codec,
}
#[pymethods]
impl NativeIdentityMap {
    #[new]
    fn new(pairs: Vec<(u32, u32)>, max_entries: usize) -> PyResult<Self> {
        Ok(Self {
            codec: Codec::try_new(&pairs, max_entries).map_err(codec_failure)?,
        })
    }
    fn encode(&self, identity: u32) -> PyResult<u32> {
        self.codec.encode(identity).map_err(codec_failure)
    }
    fn decode(&self, representation: u32) -> PyResult<u32> {
        self.codec.decode(representation).map_err(codec_failure)
    }
}
fn codec_failure(failure: CodecFailure) -> PyErr {
    NativeFailure::new_err(failure.encode())
}
#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeIdentityMap>()?;
    module.add_class::<NativeDefinition>()?;
    module.add_class::<NativeInstance>()?;
    module.add_class::<NativeDecision>()?;
    module.add_class::<NativeArena>()?;
    module.add_class::<NativeCapabilityReport>()?;
    module.add_class::<NativeSnapshot>()?;
    module.add_class::<NativeComposition>()?;
    module.add_class::<NativeRetryPolicy>()?;
    module.add_class::<NativeRetryWaits>()?;
    module.add_function(wrap_pyfunction!(propagation_limits, module)?)?;
    module.add("NativeFailure", module.py().get_type::<NativeFailure>())?;
    Ok(())
}
