//! Private Python extension; native application objects stay in visible Python
//! arenas.
mod error;
mod host;

use std::sync::{
    Arc,
    Mutex,
    MutexGuard,
    TryLockError,
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
use status_machina_bridge::build::Template;
use status_machina_bridge::runtime::{
    Decision,
    Machine,
};
use status_machina_bridge::value::Host;

fn lock<'a, T>(py: Python<'_>, mutex: &'a Mutex<T>) -> PyResult<MutexGuard<'a, T>> {
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
struct NativeInstance {
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
#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeDefinition>()?;
    module.add_class::<NativeInstance>()?;
    module.add_class::<NativeDecision>()?;
    module.add("NativeFailure", module.py().get_type::<NativeFailure>())?;
    Ok(())
}
