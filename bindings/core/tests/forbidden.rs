//! Forbidden edges cross the private transport into the kernel's one checked
//! builder path, for native specifications and portable records alike.
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
struct NoEvaluation(ArenaState);
impl Host for NoEvaluation {
    fn arena(&self) -> &ArenaState {
        &self.0
    }
    fn execute(&self, _: Request) -> Result<Response, EvaluationError> {
        panic!("construction and inspection must not execute application behavior")
    }
}

const STATE: &str = r#"{"id":ID,"terminal":TERMINAL,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]}"#;
const PREDICATE: &str = r#"{"id":7,"expression":{"kind":"predicate","implementation":{"callback":null,"reference":{"story":0,"implementation":0}}}}"#;

fn state(id: u32, terminal: bool) -> String {
    STATE
        .replace("ID", &id.to_string())
        .replace("TERMINAL", &terminal.to_string())
}
fn edge(id: u32, source: u32, event: u32, target: u32, internal: bool, guard: &str) -> String {
    format!(
        r#"{{"id":{id},"source":{source},"event":{event},"target":{target},"internal":{internal},
        "guard":{guard},"postconditions":[],"actions":[],"effects":[],"outputs":[]}}"#
    )
}
fn pair(source: u32, target: u32) -> String {
    format!(r#"{{"source":{source},"target":{target}}}"#)
}
/// Open(0) -> Sealing(1) -> Done(2), Open -> Cancelled(3), plus `extra`.
fn spec(extra: &[String], forbidden: &[String]) -> String {
    let mut transitions = vec![
        edge(0, 0, 0, 1, false, "null"),
        edge(1, 1, 1, 2, false, "null"),
        edge(2, 0, 2, 3, false, "null"),
    ];
    transitions.extend_from_slice(extra);
    format!(
        r#"{{"initial":0,"max_states":8,"max_transitions":8,"max_bindings":4,"capabilities":[],
        "states":[{},{},{},{}],"transitions":[{}],"forbidden_edges":[{}],"dispositions":[],
        "behaviors":[{{"reference":{{"story":0,"implementation":0}},"role":"condition","callback":0}}]}}"#,
        state(0, false),
        state(1, false),
        state(2, true),
        state(3, true),
        transitions.join(","),
        forbidden.join(",")
    )
}
fn failure(
    result: Result<Template<NoEvaluation>, status_machina_bridge::error::Error<NoEvaluation>>,
) -> Value {
    serde_json::to_value(result.err().expect("the definition is rejected").failure()).unwrap()
}
fn fields(value: &Value, names: &[&str]) -> Vec<Value> {
    names.iter().map(|name| value[*name].clone()).collect()
}

#[test]
fn specifications_and_records_reject_a_forbidden_edge_identically() {
    let sealing_cancelled = pair(1, 3);
    for guard in ["null", PREDICATE] {
        for event in 0..4 {
            let source = spec(
                &[edge(9, 1, event, 3, false, guard)],
                std::slice::from_ref(&sealing_cancelled),
            );
            let native = failure(Template::parse(&source));
            assert_eq!(
                fields(
                    &native,
                    &["code", "reason", "transition", "source", "target"]
                ),
                [
                    Value::from("forbidden_transition"),
                    Value::from("forbidden_transition"),
                    Value::from(9),
                    Value::from(1),
                    Value::from(3),
                ]
            );
            let record = failure(Template::bind_record(&source));
            assert_eq!(record["code"], "record");
            assert_eq!(
                fields(&record, &["reason", "transition", "source", "target"]),
                fields(&native, &["reason", "transition", "source", "target"])
            );
        }
    }
}

#[test]
fn unknown_duplicate_self_and_oversized_prohibitions_carry_their_elements() {
    let cases = [
        (
            spec(&[], &[pair(1, 8)]),
            "unknown_state",
            "unknown_forbidden_edge_state",
            [
                Value::Null,
                Value::from(1),
                Value::from(8),
                Value::from(8),
                Value::Null,
            ],
        ),
        (
            spec(&[], &[pair(1, 3), pair(1, 3)]),
            "duplicate_identifier",
            "duplicate_forbidden_edge",
            [
                Value::Null,
                Value::from(1),
                Value::from(3),
                Value::Null,
                Value::Null,
            ],
        ),
        (
            spec(&[edge(9, 1, 3, 1, true, "null")], &[pair(1, 1)]),
            "forbidden_transition",
            "forbidden_transition",
            [
                Value::from(9),
                Value::from(1),
                Value::from(1),
                Value::Null,
                Value::Null,
            ],
        ),
        (
            spec(&[], &vec![pair(0, 2); 129]),
            "definition_limit",
            "forbidden_edge_limit_exceeded",
            [
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::from(128),
            ],
        ),
    ];
    for (source, code, reason, elements) in cases {
        let native = failure(Template::parse(&source));
        assert_eq!(
            fields(&native, &["code", "reason"]),
            [Value::from(code), Value::from(reason)]
        );
        let names = ["transition", "source", "target", "state", "limit"];
        assert_eq!(fields(&native, &names), elements, "{reason}");
        let record = failure(Template::bind_record(&source));
        assert_eq!(fields(&record, &["reason"]), [Value::from(reason)]);
        assert_eq!(fields(&record, &names), elements, "{reason}");
    }
}

#[test]
fn coverage_reports_sorted_pairs_and_diagrams_omit_them() {
    let constrained =
        Template::<NoEvaluation>::parse(&spec(&[], &[pair(1, 3), pair(0, 2)])).unwrap();
    let plain = Template::<NoEvaluation>::parse(&spec(&[], &[])).unwrap();
    let coverage =
        serde_json::to_value(constrained.structural_coverage(16, 4096).unwrap()).unwrap();
    assert_eq!(
        coverage["forbidden_edges"],
        serde_json::from_str::<Value>(r#"[{"source":0,"target":2},{"source":1,"target":3}]"#)
            .unwrap()
    );
    let labels = r#"{"states":[{"id":0,"text":"Open"},{"id":1,"text":"Sealing"},{"id":2,"text":"Done"},
        {"id":3,"text":"Cancelled"}],"transitions":[{"id":0,"text":"seal"},{"id":1,"text":"finish"},
        {"id":2,"text":"cancel"}],"events":[{"id":0,"text":"Seal"},{"id":1,"text":"Finish"},
        {"id":2,"text":"Cancel"}],"conditions":[],"dispositions":[]}"#;
    assert_eq!(
        constrained.diagram(labels, 16, 4096).unwrap(),
        plain.diagram(labels, 16, 4096).unwrap()
    );
}
