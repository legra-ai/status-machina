//! Interpreter callbacks are retained only for the duration of an explicit
//! call.
use std::fmt;
use std::sync::{
    Arc,
    Mutex,
};

use pyo3::prelude::*;
use status_machina::EvaluationError;
use status_machina_bridge::callback::{
    Request,
    Response,
};
use status_machina_bridge::value::{
    ArenaState,
    Host,
};

pub(crate) struct PyHost {
    arena: ArenaState,
    active: Mutex<Option<Py<PyAny>>>,
}
impl fmt::Debug for PyHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PythonHost")
            .field("arena", &self.arena.id())
            .finish()
    }
}
impl PyHost {
    pub(crate) fn try_new(limit: usize) -> Result<Self, EvaluationError> {
        Ok(Self {
            arena: ArenaState::new(limit)?,
            active: Mutex::new(None),
        })
    }
    pub(crate) fn activate(
        self: &Arc<Self>,
        callback: Py<PyAny>,
    ) -> Result<Activation, status_machina_bridge::error::Error<Self>> {
        let mut active = self
            .active
            .lock()
            .expect("Python callback lock is not poisoned");
        if active.is_some() {
            return Err(status_machina_bridge::error::Error::Busy);
        }
        *active = Some(callback);
        Ok(Activation(Arc::clone(self)))
    }
}
pub(crate) struct Activation(Arc<PyHost>);
impl Drop for Activation {
    fn drop(&mut self) {
        self.0
            .active
            .lock()
            .expect("Python callback lock is not poisoned")
            .take();
    }
}
impl Host for PyHost {
    fn arena(&self) -> &ArenaState {
        &self.arena
    }
    fn execute(&self, request: Request) -> Result<Response, EvaluationError> {
        Python::attach(|py| {
            let callback = self
                .active
                .lock()
                .expect("Python callback lock is not poisoned")
                .as_ref()
                .ok_or_else(|| EvaluationError::new("no active Python invocation"))?
                .clone_ref(py);
            let encoded = serde_json::to_string(&request).map_err(EvaluationError::from_source)?;
            let result = callback
                .call1(py, (encoded,))
                .map_err(EvaluationError::from_source)?;
            let text = result
                .extract::<String>(py)
                .map_err(EvaluationError::from_source)?;
            if text.len() > 65_536 {
                return Err(EvaluationError::new("callback metadata exceeds 64 KiB"));
            }
            serde_json::from_str(&text).map_err(EvaluationError::from_source)
        })
    }
}
