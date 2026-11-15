//! Portable records bind through `DefinitionRecord::bind` with exact Story
//! references only, and every rejection keeps its role-specific context.
use std::sync::Arc;
use std::sync::atomic::{
    AtomicU32,
    Ordering,
};

use status_machina::EvaluationError;
use status_machina_bridge::build::Template;
use status_machina_bridge::callback::{
    CallbackKind,
    Request,
    Response,
};
use status_machina_bridge::error::Error;
use status_machina_bridge::runtime::Machine;
use status_machina_bridge::value::{
    ArenaState,
    Host,
};

/// Answers snapshots with fresh keys and every predicate with `true`.
#[derive(Debug)]
struct Kernel {
    arena: ArenaState,
    next: AtomicU32,
    calls: AtomicU32,
}
impl Host for Kernel {
    fn arena(&self) -> &ArenaState {
        &self.arena
    }
    fn execute(&self, request: Request) -> Result<Response, EvaluationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(match request.kind {
            CallbackKind::Predicate | CallbackKind::Invariant => Response::Bool(true),
            CallbackKind::Action => Response::Unit,
            _ => Response::Value(self.next.fetch_add(1, Ordering::SeqCst)),
        })
    }
}
fn kernel() -> Arc<Kernel> {
    Arc::new(Kernel {
        arena: ArenaState::new(64).unwrap(),
        next: AtomicU32::new(100),
        calls: AtomicU32::new(0),
    })
}
const REF: &str = r#"{"callback":null,"reference":{"story":0,"implementation":0}}"#;
fn state(id: u32, terminal: bool) -> String {
    format!(
        r#"{{"id":{id},"terminal":{terminal},"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]}}"#
    )
}
fn transition(guard: &str, actions: &str, internal: bool, target: u32) -> String {
    format!(
        r#"{{"id":0,"source":0,"event":0,"target":{target},"internal":{internal},"guard":{guard},"postconditions":[],"actions":{actions},"effects":[],"outputs":[]}}"#
    )
}
fn definition(states: &str, transitions: &str, dispositions: &str, behaviors: &str) -> String {
    format!(
        r#"{{"initial":0,"max_states":4,"max_transitions":4,"max_bindings":8,"capabilities":[],"states":[{states}],"transitions":[{transitions}],"dispositions":[{dispositions}],"behaviors":[{behaviors}]}}"#
    )
}
fn behavior(role: &str, story: u32, implementation: u32) -> String {
    format!(
        r#"{{"reference":{{"story":{story},"implementation":{implementation}}},"role":"{role}","callback":0}}"#
    )
}
fn predicate(id: u32) -> String {
    format!(r#"{{"id":{id},"expression":{{"kind":"predicate","implementation":{REF}}}}}"#)
}
fn bind(source: &str) -> Result<Template<Kernel>, Error<Kernel>> {
    Template::<Kernel>::bind_record(source)
}
fn failure(source: &str) -> status_machina_bridge::error::Failure {
    bind(source)
        .err()
        .expect("record must be rejected")
        .failure()
}
fn guarded(guard: &str) -> String {
    definition(
        &format!("{},{}", state(0, false), state(1, true)),
        &transition(guard, "[]", false, 1),
        "",
        &behavior("condition", 0, 0),
    )
}

#[test]
fn reference_only_record_binds_and_dispatches_through_the_registry() {
    let actions = format!(r#"[{{"id":3,"implementation":{REF}}}]"#);
    let source = definition(
        &format!("{},{}", state(0, false), state(1, true)),
        &transition(&predicate(5), &actions, false, 1),
        "",
        &[behavior("condition", 0, 0), behavior("action", 1, 0)].join(","),
    );
    // The action use selects Story 1, a distinct action Story.
    let source = source.replacen(
        r#""actions":[{"id":3,"implementation":{"callback":null,"reference":{"story":0,"implementation":0}}}]"#,
        r#""actions":[{"id":3,"implementation":{"callback":null,"reference":{"story":1,"implementation":0}}}]"#,
        1,
    );
    let template = bind(&source).unwrap();
    assert_eq!(
        template.structural_coverage(8, 1024).unwrap().states.len(),
        2
    );
    let host = kernel();
    let mut machine = Machine::try_new(&template, Arc::clone(&host), 0).unwrap();
    let before = host.calls.load(Ordering::SeqCst);
    machine.dispatch(0, 1, 2, None).unwrap();
    assert!(host.calls.load(Ordering::SeqCst) > before);
    assert_eq!(machine.state().unwrap(), 1);
}

#[test]
fn records_never_select_inline_callbacks() {
    let inline = r#"{"id":5,"expression":{"kind":"predicate","implementation":{"callback":0,"reference":null}}}"#;
    assert!(matches!(
        bind(&guarded(inline)),
        Err(Error::Construction {
            role: "predicate",
            ..
        })
    ));
    let source = guarded("null").replace(
        r#""capabilities":[]"#,
        &format!(r#""capabilities":[{{"id":0,"implementation":{REF}}}]"#),
    );
    assert!(matches!(
        bind(&source),
        Err(Error::Construction { role: "record", .. })
    ));
}

#[test]
fn story_resolution_failures_name_the_exact_references() {
    let unknown_story = guarded(&predicate(5)).replace(
        r#""story":0,"implementation":0}}}}"#,
        r#""story":7,"implementation":0}}}}"#,
    );
    let f = failure(&unknown_story);
    assert_eq!((f.code, f.story), ("unknown_behavior", Some(7)));
    let unknown_implementation = guarded(&predicate(5)).replace(
        r#""story":0,"implementation":0}}}}"#,
        r#""story":0,"implementation":9}}}}"#,
    );
    let f = failure(&unknown_implementation);
    assert_eq!(
        (f.code, f.story, f.implementation),
        ("unknown_implementation", Some(0), Some(9))
    );
    let wrong_role = definition(
        &format!("{},{}", state(0, false), state(1, true)),
        &transition(&predicate(5), "[]", false, 1),
        "",
        &behavior("action", 0, 0),
    );
    let f = failure(&wrong_role);
    assert_eq!(f.code, "role_mismatch");
    assert_eq!(
        (f.expected_role, f.actual_role),
        (Some("condition"), Some("action"))
    );
}

#[test]
fn structural_record_failures_keep_their_record_identity() {
    let mut nested = predicate(0);
    for id in 1..=32 {
        nested = format!(r#"{{"id":{id},"expression":{{"kind":"not","child":{nested}}}}}"#);
    }
    // The kernel rejects the first expression beyond depth 32: the innermost.
    let f = failure(&guarded(&nested));
    assert_eq!((f.code, f.condition), ("record_condition", Some(0)));
    let empty = r#"{"id":4,"expression":{"kind":"all","children":[]}}"#;
    assert_eq!(failure(&guarded(empty)).condition, Some(4));
    let internal = definition(
        &format!("{},{}", state(0, false), state(1, true)),
        &transition("null", "[]", true, 1),
        "",
        "",
    );
    let f = failure(&internal);
    assert_eq!((f.code, f.transition), ("record_transition", Some(0)));
    let reason = "x".repeat(257);
    let disposition = format!(
        r#"{{"id":6,"source":0,"event":1,"condition":{},"reason":"{reason}"}}"#,
        predicate(5)
    );
    let source = definition(
        &format!("{},{}", state(0, false), state(1, true)),
        &transition("null", "[]", false, 1),
        &disposition,
        &behavior("condition", 0, 0),
    );
    let f = failure(&source);
    assert_eq!((f.code, f.disposition), ("record_disposition", Some(6)));
}

#[test]
fn graph_failures_carry_native_role_indexes() {
    let unreachable = definition(
        &format!("{},{},{}", state(0, false), state(1, true), state(2, true)),
        &transition("null", "[]", false, 1),
        "",
        "",
    );
    let f = failure(&unreachable);
    assert_eq!((f.code, f.state), ("record", Some(2)));
    let unknown_target = definition(
        &state(0, false),
        &transition("null", "[]", false, 3),
        "",
        "",
    );
    let f = failure(&unknown_target);
    assert_eq!(
        (f.code, f.transition, f.state),
        ("record", Some(0), Some(3))
    );
}
