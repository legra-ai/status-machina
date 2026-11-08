//! Typed private callback requests; only compact references cross this
//! boundary.
use serde::{
    Deserialize,
    Serialize,
};
use status_machina::{
    CapabilityStatus,
    EvaluationError,
};

use crate::value::{
    Data,
    Host,
    Value,
    ValueKey,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CallbackKind {
    Snapshot,
    Predicate,
    Action,
    Construct,
    Invariant,
    Capability,
    Assertion,
}
#[derive(Debug, Serialize)]
pub struct Request {
    pub kind: CallbackKind,
    pub callback: Option<u32>,
    pub values: Vec<ValueKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<Observation>,
}
#[derive(Debug, Serialize)]
pub struct Observation {
    pub scenario: u32,
    pub step: usize,
    pub event: u32,
    pub before: u32,
    pub after: u32,
    pub kind: String,
    pub transition: Option<u32>,
    pub disposition: Option<u32>,
    pub outputs: Vec<u32>,
    pub effects: Vec<u32>,
}
impl Request {
    pub fn new(kind: CallbackKind, callback: Option<u32>, values: Vec<ValueKey>) -> Self {
        Self {
            kind,
            callback,
            values,
            state: None,
            observation: None,
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Response {
    Unit,
    Bool(bool),
    Value(u32),
    Capability(CapabilityValue),
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityValue {
    Available,
    Unavailable,
    Unknown,
}

pub(crate) fn predicate<H: Host>(
    callback: u32,
    data: &Data<H>,
    event: &Value<H>,
    facts: &Value<H>,
) -> Result<bool, EvaluationError> {
    let request = Request::new(
        CallbackKind::Predicate,
        Some(callback),
        vec![data.value.key(), event.key(), facts.key()],
    );
    boolean(data.value.host().execute(request)?)
}
pub(crate) fn boolean(response: Response) -> Result<bool, EvaluationError> {
    match response {
        Response::Bool(value) => Ok(value),
        _ => Err(EvaluationError::new("predicate must return a boolean")),
    }
}
pub(crate) fn invariant<H: Host>(callback: u32, data: &Data<H>) -> Result<bool, EvaluationError> {
    boolean(data.value.host().execute(Request::new(
        CallbackKind::Invariant,
        Some(callback),
        vec![data.value.key()],
    ))?)
}
pub(crate) fn action<H: Host>(
    callback: u32,
    data: &mut Data<H>,
    event: &Value<H>,
    facts: &Value<H>,
) -> Result<(), EvaluationError> {
    let request = Request::new(
        CallbackKind::Action,
        Some(callback),
        vec![data.value.key(), event.key(), facts.key()],
    );
    let host = std::sync::Arc::clone(data.value.host());
    match host.execute(request)? {
        Response::Unit => Ok(()),
        Response::Value(key) => {
            data.value = Value::claim(host, key)?;
            Ok(())
        }
        _ => Err(EvaluationError::new(
            "action must return no value or an explicit replacement",
        )),
    }
}
pub(crate) fn construct<H: Host>(
    callback: u32,
    data: &Data<H>,
    event: &Value<H>,
    facts: &Value<H>,
) -> Result<Value<H>, EvaluationError> {
    let request = Request::new(
        CallbackKind::Construct,
        Some(callback),
        vec![data.value.key(), event.key(), facts.key()],
    );
    let host = data.value.host();
    let Response::Value(key) = host.execute(request)? else {
        return Err(EvaluationError::new("constructor must return a value key"));
    };
    Value::claim(std::sync::Arc::clone(host), key)
}
pub(crate) fn capability<H: Host>(
    callback: u32,
    state: &u32,
    data: &Data<H>,
    facts: &Value<H>,
) -> Result<CapabilityStatus, EvaluationError> {
    let mut request = Request::new(
        CallbackKind::Capability,
        Some(callback),
        vec![data.value.key(), facts.key()],
    );
    request.state = Some(*state);
    match data.value.host().execute(request)? {
        Response::Capability(CapabilityValue::Available) => Ok(CapabilityStatus::Available),
        Response::Capability(CapabilityValue::Unavailable) => Ok(CapabilityStatus::Unavailable),
        Response::Capability(CapabilityValue::Unknown) => Ok(CapabilityStatus::Unknown),
        _ => Err(EvaluationError::new(
            "capability must return a capability status",
        )),
    }
}

pub(crate) fn assertion<H: Host>(
    callback: u32,
    observation: &status_machina::ScenarioObservation<crate::ids::Ids<H>>,
) -> Result<bool, EvaluationError> {
    use status_machina::ScenarioStepKind;
    let (kind, transition, disposition) = match observation.kind() {
        ScenarioStepKind::Transitioned(id) => ("transitioned", Some(*id), None),
        ScenarioStepKind::Ignored(id) => ("ignored", None, Some(*id)),
        ScenarioStepKind::Refused => ("refused", None, None),
        ScenarioStepKind::DispatchFailed => ("dispatch_failed", None, None),
    };
    let mut request = Request::new(CallbackKind::Assertion, Some(callback), Vec::new());
    request.observation = Some(Observation {
        scenario: observation.scenario().index,
        step: observation.step(),
        event: *observation.input_event(),
        before: *observation.before(),
        after: *observation.after(),
        kind: kind.into(),
        transition,
        disposition,
        outputs: observation.output_events().to_vec(),
        effects: observation.effects().to_vec(),
    });
    boolean(observation.scenario().host.execute(request)?)
}
