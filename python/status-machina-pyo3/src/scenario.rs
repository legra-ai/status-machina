//! Kernel scenario limits, sets and assertions; scenario identities, inputs
//! and callbacks stay in Python.
use pyo3::prelude::*;
use status_machina_bridge::scenario::{
    Failure,
    Fault,
    Limits,
    check,
    check_assertion,
};

use crate::error::{
    NativeFailure,
    encode,
    translate,
};
use crate::host::PyHost;

fn failure(failure: &Failure) -> PyErr {
    NativeFailure::new_err(failure.encode())
}
pub(crate) fn fault(py: Python<'_>, fault: Fault<PyHost>) -> PyErr {
    match fault {
        Fault::Scenario(value) => failure(&value),
        Fault::Core(error) => translate(py, error),
    }
}
/// Resolve `ScenarioLimits` through the kernel builder.
#[pyfunction]
pub(crate) fn scenario_limits(source: &str) -> PyResult<String> {
    Ok(encode(&Limits::parse(source).map_err(|e| failure(&e))?))
}
/// Check scenario-set bounds through the kernel builder.
#[pyfunction]
pub(crate) fn scenario_set(source: &str) -> PyResult<()> {
    check(source).map_err(|e| failure(&e))
}
/// Check an assertion predicate through the kernel builder.
#[pyfunction]
pub(crate) fn scenario_assertion(has_predicate: bool) -> PyResult<()> {
    check_assertion(has_predicate).map_err(|e| failure(&e))
}
