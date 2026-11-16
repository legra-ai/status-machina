//! Kernel retry policies and wait sets; wake payloads and identities stay in
//! Python.
use std::sync::Mutex;

use pyo3::prelude::*;
use status_machina_bridge::retry::{
    Failure,
    Policy,
    Waits,
};

use crate::error::{
    NativeFailure,
    encode,
};
use crate::lock;

fn failure(failure: Failure) -> PyErr {
    NativeFailure::new_err(failure.encode())
}

/// One kernel-validated `RetryPolicy`.
#[pyclass(name = "_RetryPolicy", frozen, module = "status_machina._native")]
pub(crate) struct NativeRetryPolicy {
    policy: Policy,
}
#[pymethods]
impl NativeRetryPolicy {
    #[new]
    fn new(source: &str) -> PyResult<Self> {
        Ok(Self {
            policy: Policy::parse(source).map_err(failure)?,
        })
    }
    fn schedule(&self, source: &str) -> PyResult<String> {
        Ok(encode(&self.policy.schedule(source).map_err(failure)?))
    }
}

/// One instance's kernel `RetryWaits`.
#[pyclass(name = "_RetryWaits", frozen, module = "status_machina._native")]
pub(crate) struct NativeRetryWaits {
    waits: Mutex<Waits>,
}
#[pymethods]
impl NativeRetryWaits {
    #[new]
    #[pyo3(signature = (capacity=None))]
    fn new(capacity: Option<usize>) -> PyResult<Self> {
        Ok(Self {
            waits: Mutex::new(Waits::try_new(capacity).map_err(failure)?),
        })
    }
    /// An independent copy of the kernel value, for candidate snapshots.
    fn clone(&self, py: Python<'_>) -> PyResult<Self> {
        Ok(Self {
            waits: Mutex::new(lock(py, &self.waits)?.clone()),
        })
    }
    fn schedule(&self, py: Python<'_>, source: &str) -> PyResult<String> {
        lock(py, &self.waits)?.schedule(source).map_err(failure)
    }
    fn pending(&self, py: Python<'_>) -> PyResult<String> {
        Ok(encode(&lock(py, &self.waits)?.pending()))
    }
    fn get(&self, py: Python<'_>, operation: Option<u32>) -> PyResult<String> {
        Ok(encode(&lock(py, &self.waits)?.get(operation)))
    }
    fn mark_registered(&self, py: Python<'_>, source: &str) -> PyResult<bool> {
        lock(py, &self.waits)?
            .mark_registered(source)
            .map_err(failure)
    }
    fn is_due(&self, py: Python<'_>, source: &str) -> PyResult<bool> {
        lock(py, &self.waits)?.is_due(source).map_err(failure)
    }
    fn consume(&self, py: Python<'_>, source: &str) -> PyResult<String> {
        Ok(encode(
            &lock(py, &self.waits)?.consume(source).map_err(failure)?,
        ))
    }
    fn cancel(&self, py: Python<'_>, operation: Option<u32>) -> PyResult<String> {
        Ok(encode(&lock(py, &self.waits)?.cancel(operation)))
    }
}
