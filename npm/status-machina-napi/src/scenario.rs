//! Kernel scenario limits, sets and assertions; scenario identities, inputs
//! and callbacks stay in JavaScript.
use napi_derive::napi;
use status_machina_bridge::scenario::{
    Failure,
    Fault,
    Limits,
    check,
    check_assertion,
};

use crate::error::{
    encode,
    translate,
};
use crate::host::JsHost;

fn failure(failure: &Failure) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, failure.encode())
}
pub(crate) fn fault(fault: Fault<JsHost>) -> napi::Error {
    match fault {
        Fault::Scenario(value) => failure(&value),
        Fault::Core(error) => translate(error),
    }
}
/// Resolve `ScenarioLimits` through the kernel builder.
#[napi]
pub fn scenario_limits(source: String) -> napi::Result<String> {
    Ok(encode(&Limits::parse(&source).map_err(|e| failure(&e))?))
}
/// Check scenario-set bounds through the kernel builder.
#[napi]
pub fn scenario_set(source: String) -> napi::Result<()> {
    check(&source).map_err(|e| failure(&e))
}
/// Check an assertion predicate through the kernel builder.
#[napi]
pub fn scenario_assertion(has_predicate: bool) -> napi::Result<()> {
    check_assertion(has_predicate).map_err(|e| failure(&e))
}
