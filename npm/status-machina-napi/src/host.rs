//! Each invocation uses only the Env supplied on its owning Node thread.
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::thread::{
    self,
    ThreadId,
};

use napi::Env;
use napi::bindgen_prelude::{
    Function,
    FunctionRef,
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

use crate::error::{
    CoreError,
    JsCause,
    translate,
};

struct Invocation {
    env: Env,
    callback: FunctionRef<String, String>,
}
pub(crate) struct JsHost {
    arena: ArenaState,
    environment: usize,
    thread: ThreadId,
    active: RefCell<Option<Rc<Invocation>>>,
}
impl fmt::Debug for JsHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeHost")
            .field("arena", &self.arena.id())
            .finish()
    }
}
impl JsHost {
    pub(crate) fn try_new(env: Env, capacity: usize) -> Result<Self, EvaluationError> {
        Ok(Self {
            arena: ArenaState::new(capacity)?,
            environment: env.raw() as usize,
            thread: thread::current().id(),
            active: RefCell::new(None),
        })
    }
    pub(crate) fn check(&self, env: Env) -> napi::Result<()> {
        if self.environment != env.raw() as usize || self.thread != thread::current().id() {
            return Err(translate(CoreError::construction(
                "environment",
                "instance belongs to another Node environment",
            )));
        }
        Ok(())
    }
    pub(crate) fn activate(
        self: &Arc<Self>,
        env: Env,
        callback: Function<'_, String, String>,
    ) -> napi::Result<Activation> {
        self.check(env)?;
        if self.active.borrow().is_some() {
            return Err(translate(CoreError::Busy));
        }
        let callback = callback.create_ref()?;
        *self.active.borrow_mut() = Some(Rc::new(Invocation { env, callback }));
        Ok(Activation(Arc::clone(self)))
    }
}
pub(crate) struct Activation(Arc<JsHost>);
impl Drop for Activation {
    fn drop(&mut self) {
        self.0.active.borrow_mut().take();
    }
}
impl Host for JsHost {
    fn arena(&self) -> &ArenaState {
        &self.arena
    }
    fn execute(&self, request: Request) -> Result<Response, EvaluationError> {
        let invocation = self
            .active
            .borrow()
            .clone()
            .ok_or_else(|| EvaluationError::new("no active Node invocation"))?;
        let encoded = serde_json::to_string(&request).map_err(EvaluationError::from_source)?;
        let callback = invocation
            .callback
            .borrow_back(&invocation.env)
            .map_err(|e| EvaluationError::from_source(JsCause::new(e)))?;
        let result = callback
            .call(encoded)
            .map_err(|e| EvaluationError::from_source(JsCause::new(e)))?;
        if result.len() > 65_536 {
            return Err(EvaluationError::new("callback metadata exceeds 64 KiB"));
        }
        serde_json::from_str(&result).map_err(EvaluationError::from_source)
    }
}
