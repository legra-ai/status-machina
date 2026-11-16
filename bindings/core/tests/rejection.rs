//! Construction rejections cross as typed categories with the kernel variant
//! and the rejected element, never as message text alone.
use serde_json::Value;
use status_machina::EvaluationError;
use status_machina_bridge::build::Template;
use status_machina_bridge::callback::{
    Request,
    Response,
};
use status_machina_bridge::value::{
    ArenaState,
    Host,
};

#[derive(Debug)]
struct NativeHost {
    arena: ArenaState,
}
impl Host for NativeHost {
    fn arena(&self) -> &ArenaState {
        &self.arena
    }
    fn execute(&self, _: Request) -> Result<Response, EvaluationError> {
        Err(EvaluationError::new("no callback runs during construction"))
    }
}
const STATE: &str = r#"{"id":ID,"terminal":false,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]}"#;
fn state(id: u32) -> String {
    STATE.replace("ID", &id.to_string())
}
fn transition(id: u32, target: u32) -> String {
    format!(
        r#"{{"id":{id},"source":0,"event":0,"target":{target},"internal":false,"guard":null,
        "postconditions":[],"actions":[],"effects":[],"outputs":[]}}"#
    )
}
fn spec(states: &[String], transitions: &[String], behaviors: &str) -> String {
    format!(
        r#"{{"initial":0,"max_states":128,"max_transitions":512,"max_bindings":128,"capabilities":[],
        "states":[{}],"transitions":[{}],"dispositions":[],"behaviors":{behaviors}}}"#,
        states.join(","),
        transitions.join(",")
    )
}
fn failure(source: &str) -> Value {
    let error = Template::<NativeHost>::parse(source)
        .err()
        .expect("the specification is rejected");
    serde_json::to_value(error.failure()).unwrap()
}
fn fields(value: &Value, names: &[&str]) -> Vec<Value> {
    names.iter().map(|name| value[*name].clone()).collect()
}

#[test]
fn graph_rules_carry_their_category_variant_and_element() {
    let unreachable = failure(&spec(&[state(0), state(1)], &[], "[]"));
    assert_eq!(
        fields(&unreachable, &["code", "reason", "state"]),
        [
            Value::from("unreachable_state"),
            Value::from("unreachable_state"),
            Value::from(1)
        ]
    );
    let unknown = failure(&spec(&[state(0)], &[transition(4, 9)], "[]"));
    assert_eq!(
        fields(&unknown, &["code", "reason", "transition", "state"]),
        [
            Value::from("unknown_state"),
            Value::from("unknown_transition_state"),
            Value::from(4),
            Value::from(9)
        ]
    );
}

#[test]
fn builder_rules_carry_the_rejected_element() {
    let internal = spec(
        &[state(0), state(1)],
        &[transition(3, 1).replace(r#""internal":false"#, r#""internal":true"#)],
        "[]",
    );
    assert_eq!(
        fields(&failure(&internal), &["code", "reason", "transition"]),
        [
            Value::from("internal_transition"),
            Value::from("internal_transition_must_be_self"),
            Value::from(3)
        ]
    );
    let empty = spec(
        &[state(0)],
        &[transition(0, 0).replace(
            r#""guard":null"#,
            r#""guard":{"id":5,"expression":{"kind":"all","children":[]}}"#,
        )],
        "[]",
    );
    assert_eq!(
        fields(&failure(&empty), &["code", "reason", "condition"]),
        [
            Value::from("invalid_condition"),
            Value::from("empty_group"),
            Value::from(5)
        ]
    );
}

#[test]
fn unresolved_story_references_are_typed_with_their_element() {
    let reference = r#"{"callback":null,"reference":{"story":2,"implementation":0}}"#;
    let action = format!(r#"[{{"id":6,"implementation":{reference}}}]"#);
    let source = spec(
        &[state(0)],
        &[transition(0, 0).replace(r#""actions":[]"#, &format!(r#""actions":{action}"#))],
        r#"[{"reference":{"story":2,"implementation":0},"role":"condition","callback":0}]"#,
    );
    assert_eq!(
        fields(
            &failure(&source),
            &[
                "code",
                "reason",
                "action",
                "story",
                "expected_role",
                "actual_role"
            ]
        ),
        [
            Value::from("role_mismatch"),
            Value::from("signature_mismatch"),
            Value::from(6),
            Value::from(2),
            Value::from("action"),
            Value::from("condition")
        ]
    );
    let duplicate = spec(
        &[state(0)],
        &[],
        r#"[{"reference":{"story":2,"implementation":0},"role":"action","callback":0},
            {"reference":{"story":2,"implementation":0},"role":"action","callback":1}]"#,
    );
    assert_eq!(
        fields(&failure(&duplicate), &["code", "story", "implementation"]),
        [
            Value::from("duplicate_implementation"),
            Value::from(2),
            Value::from(0)
        ]
    );
}
