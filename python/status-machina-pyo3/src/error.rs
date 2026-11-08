//! Preserve original Python exception objects across fallible kernel
//! evaluation.
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

use crate::host::PyHost;

create_exception!(_native, NativeFailure, PyException);
pub(crate) type CoreError = status_machina_bridge::error::Error<PyHost>;
pub(crate) fn translate(py: Python<'_>, error: CoreError) -> PyErr {
    let text = serde_json::to_string(&error.failure())
        .expect("typed native failure metadata is serializable");
    let translated = NativeFailure::new_err(text);
    if let Some(source) = error
        .cause()
        .and_then(|e| e.cause())
        .and_then(|e| e.downcast_ref::<PyErr>())
    {
        translated.set_cause(py, Some(source.clone_ref(py)));
    }
    translated
}
pub(crate) fn encode<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("typed native result metadata is serializable")
}
