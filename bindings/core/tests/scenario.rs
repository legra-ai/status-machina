//! Scenario limits, sets and replay keep the kernel contract across the
//! private transport, with typed rejections and original callback causes.
use std::sync::Arc;
use std::sync::atomic::{
    AtomicU32,
    Ordering,
};

use status_machina::{
    EvaluationError,
    EvaluationPhase,
};
use status_machina_bridge::build::Template;
use status_machina_bridge::callback::{
    CallbackKind,
    Request,
    Response,
};
use status_machina_bridge::error::Error;
use status_machina_bridge::scenario::{
    Failure,
    Fault,
    Limits,
    check,
    check_assertion,
};
use status_machina_bridge::value::{
    ArenaState,
    Host,
};

/// Snapshots return fresh keys; assertion callback 0 holds unless the step
/// transitioned, and callback 1 raises.
#[derive(Debug)]
struct NativeHost {
    arena: ArenaState,
    next: AtomicU32,
}
impl Host for NativeHost {
    fn arena(&self) -> &ArenaState {
        &self.arena
    }
    fn execute(&self, request: Request) -> Result<Response, EvaluationError> {
        match request.kind {
            CallbackKind::Snapshot => Ok(Response::Value(self.next.fetch_add(1, Ordering::SeqCst))),
            CallbackKind::Assertion if request.callback == Some(1) => {
                Err(EvaluationError::new("assertion raised"))
            }
            CallbackKind::Assertion => {
                let observation = request.observation.expect("assertions observe a step");
                Ok(Response::Bool(observation.kind != "transitioned"))
            }
            _ => Err(EvaluationError::new("unexpected callback")),
        }
    }
}
fn host() -> Arc<NativeHost> {
    Arc::new(NativeHost {
        arena: ArenaState::new(64).unwrap(),
        next: AtomicU32::new(100),
    })
}
const SPEC: &str = r#"{
 "initial":0,"max_states":128,"max_transitions":512,"max_bindings":128,"capabilities":[],
 "states":[
  {"id":0,"terminal":false,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]},
  {"id":1,"terminal":true,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]}
 ],
 "transitions":[{"id":0,"source":0,"event":0,"target":1,"internal":false,"guard":null,
 "postconditions":[],"actions":[],"effects":[],"outputs":[]}],"dispositions":[],"behaviors":[]
}"#;
fn template() -> Template<NativeHost> {
    Template::parse(SPEC).unwrap()
}
/// One scenario per entry of `events`; every payload and fact shares key 1.
fn replay(events: &[&[u32]], assertions: &str) -> Result<String, Fault<NativeHost>> {
    let scenarios: Vec<String> = events
        .iter()
        .map(|steps| {
            let steps: Vec<String> = steps
                .iter()
                .map(|event| format!(r#"{{"event":{event},"payload":1,"facts":1}}"#))
                .collect();
            format!(r#"{{"steps":[{}]}}"#, steps.join(","))
        })
        .collect();
    let source = format!(
        r#"{{"limits":{{}},"initial":0,"scenarios":[{}],"assertions":{assertions}}}"#,
        scenarios.join(",")
    );
    let host = host();
    let report = template().replay(&host, &source)?;
    assert!(
        host.arena().live().is_empty(),
        "replay released every lease"
    );
    Ok(serde_json::to_string(&report).unwrap())
}
fn scenario_failure(fault: Fault<NativeHost>) -> Failure {
    match fault {
        Fault::Scenario(failure) => *failure,
        Fault::Core(error) => panic!("expected a scenario failure, got {error}"),
    }
}
fn reason(failure: &Failure) -> (&'static str, &'static str, Option<usize>) {
    (failure.family, failure.reason, failure.limit)
}
fn entry(failure: &Failure) -> (&'static str, Option<usize>) {
    (failure.reason, failure.entry)
}

#[test]
fn limits_resolve_kernel_defaults_and_reject_unsupported_bounds() {
    assert_eq!(
        serde_json::to_string(&Limits::parse("{}").unwrap()).unwrap(),
        r#"{"max_scenarios":64,"max_steps_per_scenario":256,"max_nodes":4096,"max_outputs":4096,"max_trace_entries":4096}"#
    );
    for invalid in [
        r#"{"max_scenarios":0}"#,
        r#"{"max_nodes":65537}"#,
        r#"{"max_trace_entries":18446744073709551615}"#,
    ] {
        assert_eq!(
            reason(&Limits::parse(invalid).unwrap_err()),
            ("limits", "outside_supported_range", None)
        );
    }
    assert_eq!(
        reason(&Limits::parse(r#"{"max_scenarios":"1"}"#).unwrap_err()),
        ("transport", "malformed", None)
    );
}

#[test]
fn set_bounds_follow_the_kernel_builder_precedence() {
    let set =
        |limits: &str, steps: &str| check(&format!(r#"{{"limits":{limits},"steps":{steps}}}"#));
    assert!(set("{}", "[0,3]").is_ok());
    assert_eq!(
        reason(&set("{}", "[]").unwrap_err()),
        ("set", "empty", None)
    );
    assert_eq!(
        reason(&set(r#"{"max_scenarios":1}"#, "[1,1]").unwrap_err()),
        ("set", "scenario_limit_exceeded", Some(1))
    );
    assert_eq!(
        reason(&set(r#"{"max_steps_per_scenario":2}"#, "[3]").unwrap_err()),
        ("set", "step_limit_exceeded", Some(2))
    );
    // Each step checks the per-scenario cap before the aggregate node budget.
    assert_eq!(
        reason(&set(r#"{"max_steps_per_scenario":2,"max_nodes":4}"#, "[2,3]").unwrap_err()),
        ("set", "step_limit_exceeded", Some(2))
    );
    assert_eq!(
        reason(&set(r#"{"max_steps_per_scenario":2,"max_nodes":2}"#, "[2,3]").unwrap_err()),
        ("set", "total_node_limit_exceeded", Some(2))
    );
    assert_eq!(
        reason(&set(r#"{"max_nodes":3}"#, "[2,2]").unwrap_err()),
        ("set", "total_node_limit_exceeded", Some(3))
    );
    assert_eq!(
        entry(&set(r#"{"max_nodes":3}"#, "[1,1,2]").unwrap_err()),
        ("total_node_limit_exceeded", Some(2))
    );
    assert_eq!(entry(&set("{}", "[]").unwrap_err()), ("empty", None));
    assert!(check_assertion(true).is_ok());
    assert_eq!(
        reason(&check_assertion(false).unwrap_err()),
        ("assertion", "missing_predicate", None)
    );
}

#[test]
fn replay_completes_and_reports_counterexamples_by_position() {
    assert_eq!(
        replay(&[&[1], &[1, 1]], "[]").unwrap(),
        r#"{"kind":"complete_within_scenarios","scenarios":2,"nodes":3}"#
    );
    let inline = r#"[{"id":7,"implementation":{"callback":0,"reference":null}}]"#;
    assert_eq!(
        replay(&[&[1], &[1, 0]], inline).unwrap(),
        concat!(
            r#"{"kind":"counterexample","scenario":1,"assertion":7,"steps":["#,
            r#"{"scenario":1,"step":0,"event":1,"before":0,"after":0,"kind":"refused","transition":null,"disposition":null,"outputs":[],"effects":[]},"#,
            r#"{"scenario":1,"step":1,"event":0,"before":0,"after":1,"kind":"transitioned","transition":0,"disposition":null,"outputs":[],"effects":[]}]}"#
        )
    );
}

#[test]
fn replay_rejects_invalid_assertions_with_their_entry() {
    let missing = r#"[{"id":0,"implementation":{"callback":0,"reference":null}},{"id":1,"implementation":null}]"#;
    let failure = scenario_failure(replay(&[&[0]], missing).unwrap_err());
    assert_eq!(
        (reason(&failure), failure.entry),
        (("assertion", "missing_predicate", None), Some(1))
    );
    let unknown = r#"[{"id":0,"implementation":{"callback":null,"reference":{"story":4,"implementation":0}}}]"#;
    let failure = scenario_failure(replay(&[&[0]], unknown).unwrap_err());
    assert_eq!(
        (reason(&failure), failure.entry),
        (("assertion", "unknown_behavior", None), Some(0))
    );
    let many: Vec<String> = (0..129)
        .map(|id| format!(r#"{{"id":{id},"implementation":{{"callback":0,"reference":null}}}}"#))
        .collect();
    let failure = scenario_failure(replay(&[&[0]], &format!("[{}]", many.join(","))).unwrap_err());
    assert_eq!(
        reason(&failure),
        ("replay", "assertion_limit_exceeded", Some(128))
    );
}

#[test]
fn a_raising_assertion_stops_replay_with_its_original_cause() {
    let raising = r#"[{"id":0,"implementation":{"callback":1,"reference":null}}]"#;
    match replay(&[&[0]], raising).unwrap_err() {
        Fault::Core(
            error @ Error::Evaluation {
                phase: EvaluationPhase::Assertion,
                ..
            },
        ) => assert_eq!(error.cause().unwrap().to_string(), "assertion raised"),
        other => panic!("expected an assertion evaluation failure, got {other:?}"),
    }
}
