//! Private inspection transport is checked before infallible label lookups.
use status_machina::EvaluationError;
use status_machina_bridge::build::Template;
use status_machina_bridge::callback::{
    Request,
    Response,
};
use status_machina_bridge::error::Error;
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
        panic!("inspection must not execute native application behavior")
    }
}
const SPEC: &str = r#"{
 "initial":0,"max_states":2,"max_transitions":1,"max_bindings":1,"capabilities":[],
 "states":[
  {"id":0,"terminal":false,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]},
  {"id":1,"terminal":true,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]}
 ],
 "transitions":[{"id":0,"source":0,"event":0,"target":1,"internal":false,"guard":null,
 "postconditions":[],"actions":[],"effects":[],"outputs":[]}],"dispositions":[],"behaviors":[]
}"#;
const LABELS: &str = r#"{
 "states":[{"id":0,"text":"Same"},{"id":1,"text":"Same"}],
 "transitions":[{"id":0,"text":"Edge"}],"events":[{"id":0,"text":"Event"}],
 "conditions":[],"dispositions":[]
}"#;
#[test]
fn checked_private_labels_render_distinct_states_without_evaluation() {
    let template = Template::<NoEvaluation>::parse(SPEC).unwrap();
    let coverage = template.structural_coverage(3, 1024).unwrap();
    assert_eq!(coverage.states.len(), 2);
    let diagram = template.diagram(LABELS, 3, 1024).unwrap();
    assert_eq!(diagram.matches("state \"Same\"").count(), 2);
    assert!(diagram.contains("s0 --> s1"));
}
#[test]
fn duplicate_missing_unknown_and_malformed_private_labels_are_errors() {
    let template = Template::<NoEvaluation>::parse(SPEC).unwrap();
    for source in [
        LABELS.replace("\"id\":1", "\"id\":0"),
        LABELS.replace("\"id\":1", "\"id\":999"),
        LABELS.replace("\"conditions\":[]", "\"conditions\":[],\"unknown\":[]"),
        LABELS.replace(
            "\"states\":[{\"id\":0,\"text\":\"Same\"},{\"id\":1,\"text\":\"Same\"}]",
            "\"states\":[]",
        ),
        "not JSON".into(),
    ] {
        assert!(matches!(
            template.diagram(&source, 3, 1024),
            Err(Error::Inspection { .. })
        ));
    }
}
#[test]
fn graph_and_output_limits_retain_the_exact_budget() {
    let template = Template::<NoEvaluation>::parse(SPEC).unwrap();
    assert_eq!(
        template
            .structural_coverage(2, 1024)
            .err()
            .unwrap()
            .failure()
            .limit,
        Some(2)
    );
    assert_eq!(
        template
            .diagram(LABELS, 3, 10)
            .err()
            .unwrap()
            .failure()
            .limit,
        Some(10)
    );
    assert!(template.structural_coverage(0, 1024).is_err());
    assert!(template.diagram(LABELS, 3, 1_048_577).is_err());
}
