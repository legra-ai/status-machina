//! Native composition lowers to the kernel's registries, routes, bindings and
//! propagation, with positions and addresses for native error context.
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
use status_machina_bridge::composition::{
    Failure,
    Fault,
    Item,
    Topology,
    limits,
};
use status_machina_bridge::runtime::Machine;
use status_machina_bridge::value::{
    ArenaState,
    Host,
};

#[derive(Debug)]
struct TestHost {
    arena: ArenaState,
    next: AtomicU32,
    failing_actions: bool,
}
impl Host for TestHost {
    fn arena(&self) -> &ArenaState {
        &self.arena
    }
    fn execute(&self, request: Request) -> Result<Response, EvaluationError> {
        if self.failing_actions && matches!(request.kind, CallbackKind::Action) {
            return Err(EvaluationError::new("action refused"));
        }
        Ok(Response::Value(self.next.fetch_add(1, Ordering::SeqCst)))
    }
}
fn host(failing_actions: bool) -> Arc<TestHost> {
    Arc::new(TestHost {
        arena: ArenaState::new(64).unwrap(),
        next: AtomicU32::new(10),
        failing_actions,
    })
}
const STATES: &str = r#""states":[
  {"id":0,"terminal":false,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]},
  {"id":1,"terminal":false,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]}]"#;
/// Event 0 moves state 0 to 1 and back; every step emits output event 1.
fn emitter() -> Template<TestHost> {
    Template::parse(&format!(
        r#"{{"initial":0,"max_states":8,"max_transitions":8,"max_bindings":8,"capabilities":[],{STATES},
 "transitions":[
  {{"id":0,"source":0,"event":0,"target":1,"internal":false,"guard":null,"postconditions":[],"actions":[],"effects":[],
    "outputs":[{{"id":1,"implementation":{{"callback":0,"reference":null}}}}]}},
  {{"id":1,"source":1,"event":0,"target":0,"internal":false,"guard":null,"postconditions":[],"actions":[],"effects":[],
    "outputs":[{{"id":1,"implementation":{{"callback":0,"reference":null}}}}]}}],
 "forbidden_edges":[],"dispositions":[],"behaviors":[]}}"#
    ))
    .unwrap()
}
/// Event 0 moves state 0 to 1 through one action and one effect.
fn receiver() -> Template<TestHost> {
    Template::parse(&format!(
        r#"{{"initial":0,"max_states":8,"max_transitions":8,"max_bindings":8,"capabilities":[],{STATES},
 "transitions":[{{"id":0,"source":0,"event":0,"target":1,"internal":false,"guard":null,"postconditions":[],
   "actions":[{{"id":0,"implementation":{{"callback":1,"reference":null}}}}],
   "effects":[{{"id":0,"implementation":{{"callback":2,"reference":null}}}}],"outputs":[]}}],
 "forbidden_edges":[],"dispositions":[],"behaviors":[]}}"#
    ))
    .unwrap()
}
fn pair(failing: bool) -> (Machine<TestHost>, Machine<TestHost>) {
    (
        Machine::try_new(&emitter(), host(false), 0).unwrap(),
        Machine::try_new(&receiver(), host(failing), 0).unwrap(),
    )
}
const TOPOLOGY: &str = r#"{"addresses":[{"scope":0,"machine":0},{"scope":0,"machine":1}],
 "sources":{"max_instances":8,"entries":[{"address":0,"machine":0}]},
 "destinations":{"max_instances":8,"entries":[{"address":1,"machine":1}]},
 "routes":[{"source":0,"event":1,"destination":1,"input_event":0,"capability":null}],
 "max_routes":8,"max_batch":2}"#;
fn kernel(fault: Fault<TestHost>) -> Failure {
    match fault {
        Fault::Composition(failure) => *failure,
        Fault::Core(error) => panic!("expected a composition failure, got {error}"),
    }
}
fn propagation(steps: usize, inputs: &str) -> String {
    format!(
        r#"{{"binding":{{"extra":[],"addresses":[0,1]}},"inputs":{inputs},"facts":0,"max_steps":{steps},"max_intents":8}}"#
    )
}

#[test]
fn propagation_routes_outputs_to_the_destination_in_fifo_order() {
    let (mut first, mut second) = pair(false);
    let topology = Topology::try_new(TOPOLOGY, &[&first, &second]).unwrap();
    let scratch = host(false);
    let source = propagation(8, r#"[{"destination":0,"event":0,"payload":1}]"#);
    let propagated = topology
        .propagate(&mut [&mut first, &mut second], &scratch, &source, None)
        .unwrap();
    let report = serde_json::to_value(&propagated.report).unwrap();
    assert!(propagated.failure.is_none());
    assert_eq!(report["status"], "quiescent");
    assert_eq!(report["steps"][0]["input"], 0);
    assert_eq!(report["steps"][1]["input"], serde_json::Value::Null);
    assert_eq!(report["steps"][1]["destination"], 1);
    assert_eq!(report["effects"][0]["source"], 1);
    assert!(report["pending"].as_array().unwrap().is_empty());
    assert_eq!((first.state().unwrap(), second.state().unwrap()), (1, 1));
}

#[test]
fn later_failure_keeps_earlier_accepted_steps_and_the_failing_input() {
    let (mut first, mut second) = pair(true);
    let topology = Topology::try_new(TOPOLOGY, &[&first, &second]).unwrap();
    let source = propagation(8, r#"[{"destination":0,"event":0,"payload":1}]"#);
    let propagated = topology
        .propagate(&mut [&mut first, &mut second], &host(false), &source, None)
        .unwrap();
    let report = serde_json::to_value(&propagated.report).unwrap();
    assert_eq!(report["status"], "dispatch_failed");
    assert_eq!(report["steps"].as_array().unwrap().len(), 1);
    assert_eq!(report["pending"][0]["input"], serde_json::Value::Null);
    assert_eq!(report["pending"][0]["destination"], 1);
    assert!(propagated.failure.unwrap().cause().is_some());
    assert_eq!(
        (first.revision().unwrap(), second.revision().unwrap()),
        (1, 0)
    );
}

#[test]
fn step_limit_returns_unconsumed_caller_inputs_by_position() {
    let (mut first, mut second) = pair(false);
    let topology = Topology::try_new(TOPOLOGY, &[&first, &second]).unwrap();
    let inputs =
        r#"[{"destination":0,"event":0,"payload":1},{"destination":0,"event":0,"payload":2}]"#;
    let propagated = topology
        .propagate(
            &mut [&mut first, &mut second],
            &host(false),
            &propagation(1, inputs),
            None,
        )
        .unwrap();
    let report = serde_json::to_value(&propagated.report).unwrap();
    assert_eq!(report["status"], "step_limit");
    assert_eq!(report["pending"][0]["input"], 1);
    assert_eq!(report["pending"][1]["input"], serde_json::Value::Null);
}

#[test]
fn registry_and_route_rejections_carry_their_positions() {
    let (first, second) = pair(false);
    let mismatched = TOPOLOGY.replace(
        r#"{"scope":0,"machine":1}"#,
        r#"{"scope":0,"machine":null}"#,
    );
    let failure = kernel(
        Topology::try_new(&mismatched, &[&first, &second])
            .err()
            .unwrap(),
    );
    assert_eq!(
        (
            failure.family,
            failure.reason,
            failure.registry,
            failure.entry
        ),
        (
            "registry",
            "address_mismatch",
            Some("destinations"),
            Some(0)
        )
    );
    let routes = TOPOLOGY.replace(
        r#""routes":[{"source":0,"event":1,"destination":1,"input_event":0,"capability":null}]"#,
        r#""routes":[{"source":0,"event":1,"destination":1,"input_event":0,"capability":null},
          {"source":0,"event":0,"destination":1,"input_event":0,"capability":null}]"#,
    );
    let failure = kernel(
        Topology::try_new(&routes, &[&first, &second])
            .err()
            .unwrap(),
    );
    assert_eq!(
        (failure.reason, failure.route),
        ("undeclared_source_event", Some(1))
    );
    let limits = TOPOLOGY.replace(r#""max_batch":2"#, r#""max_batch":0"#);
    let failure = kernel(
        Topology::try_new(&limits, &[&first, &second])
            .err()
            .unwrap(),
    );
    assert_eq!((failure.reason, failure.route), ("invalid_limits", None));
}

#[test]
fn route_batch_preserves_order_and_refuses_oversized_batches() {
    let (first, second) = pair(false);
    let topology = Topology::try_new(TOPOLOGY, &[&first, &second]).unwrap();
    let scratch = host(false);
    let batch =
        r#"{"extra":[],"source":0,"outputs":[{"event":1,"payload":0},{"event":0,"payload":1}]}"#;
    let items = topology.route_batch(&scratch, batch, None).unwrap();
    assert!(matches!(
        items[..],
        [
            Item::Routed {
                destination: Some(1),
                event: 0,
                ..
            },
            Item::Unmatched { event: 0, .. }
        ]
    ));
    drop(items);
    let oversized = r#"{"extra":[],"source":0,"outputs":[{"event":1,"payload":0},{"event":1,"payload":1},{"event":1,"payload":2}]}"#;
    let failure = kernel(
        topology
            .route_batch(&scratch, oversized, None)
            .err()
            .unwrap(),
    );
    assert_eq!(
        (failure.reason, failure.limit),
        ("batch_limit_exceeded", Some(2))
    );
    let unknown = r#"{"extra":[{"scope":4294967295,"report":null}],"source":2,"outputs":[]}"#;
    let failure = kernel(topology.route_batch(&scratch, unknown, None).err().unwrap());
    assert_eq!(failure.reason, "unregistered_source");
}

#[test]
fn binding_rejections_name_the_entry_or_the_missing_address() {
    let (mut first, mut second) = pair(false);
    let topology = Topology::try_new(TOPOLOGY, &[&first, &second]).unwrap();
    let missing = r#"{"extra":[],"addresses":[0]}"#;
    let failure = kernel(topology.bind(&mut [&mut first], missing).err().unwrap());
    assert_eq!(
        (failure.reason, failure.address),
        ("missing_instance", Some(1))
    );
    let swapped = r#"{"extra":[],"addresses":[1,0]}"#;
    let failure = kernel(
        topology
            .bind(&mut [&mut first, &mut second], swapped)
            .err()
            .unwrap(),
    );
    assert_eq!(
        (failure.reason, failure.entry),
        ("identity_or_inventory_mismatch", Some(0))
    );
    assert!(
        topology
            .bind(
                &mut [&mut first, &mut second],
                r#"{"extra":[],"addresses":[0,1]}"#
            )
            .is_ok()
    );
    for (steps, intents) in [(None, Some(1)), (Some(0), Some(1)), (Some(1), Some(4097))] {
        assert_eq!(limits(steps, intents).err().unwrap().family, "limits");
    }
}
