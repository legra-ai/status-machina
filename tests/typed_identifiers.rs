//! Caller-defined identities are values, independent of their display or
//! encoding.

use std::time::{
    Duration,
    SystemTime,
};

use status_machina::{
    Capability,
    CapabilitySnapshot,
    CapabilityStatus,
    Composition,
    DispatchOutcome,
    Event,
    InstanceAddress,
    InstanceRegistry,
    MachineDefinition,
    MachineIds,
    OutputEvent,
    RetryWaits,
    Route,
    RoutedItem,
    ScenarioAssertion,
    ScenarioLimits,
    ScenarioReplay,
    ScenarioResult,
    ScenarioSet,
    StateDefinition,
    Transition,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct OrderIds;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum State {
    Pending,
    Complete,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Input {
    Finish,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Instance(u64);

impl MachineIds for OrderIds {
    type Definition = u16;
    type Instance = Instance;
    type State = State;
    type Event = Input;
    type Transition = u8;
    type Action = u8;
    type Condition = u8;
    type Effect = u8;
    type Disposition = u8;
    type Capability = u8;
    type Operation = u8;
    type Scenario = u8;
    type Assertion = u8;
    type Scope = u8;
    type Behavior = u16;
    type Implementation = u16;
}

#[test]
fn enum_states_and_events_dispatch_without_display_serde_or_uuid() {
    let transition = Transition::<(), (), (), (), (), OrderIds>::builder(1)
        .from(State::Pending)
        .on(Input::Finish)
        .to(State::Complete)
        .build()
        .unwrap();
    let definition = MachineDefinition::<(), (), (), (), (), OrderIds>::builder(42)
        .initial(State::Pending)
        .state(StateDefinition::builder(State::Pending).build())
        .unwrap()
        .state(StateDefinition::builder(State::Complete).build())
        .unwrap()
        .transition(transition)
        .unwrap()
        .build()
        .unwrap();
    let mut instance = definition.instantiate(Instance(7), ()).unwrap();
    let outcome = instance
        .dispatch(Event::new(Input::Finish, ()), &())
        .unwrap();
    assert!(outcome.is_transitioned());
    assert_eq!(instance.state(), &State::Complete);
    assert_eq!(instance.instance_id(), Instance(7));
    assert_eq!(definition.id(), &42);
}

fn source_definition() -> MachineDefinition<(), (), (), (), u8, OrderIds> {
    let transition = Transition::builder_with_output_events(1)
        .from(State::Pending)
        .on(Input::Finish)
        .to(State::Complete)
        .output_event(
            OutputEvent::builder(Input::Finish)
                .describe(|_: &(), _: &(), _: &()| 23_u8)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    MachineDefinition::builder_with_output_events(42)
        .initial(State::Pending)
        .state(StateDefinition::builder(State::Pending).build())
        .unwrap()
        .state(StateDefinition::builder(State::Complete).build())
        .unwrap()
        .transition(transition)
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn typed_ids_survive_capability_checks_and_owned_routing() {
    let source_definition = source_definition();
    let mut source = source_definition.instantiate(Instance(1), ()).unwrap();
    let destination_definition = MachineDefinition::<(), u8, (), (), (), OrderIds>::builder(43)
        .initial(State::Pending)
        .state(StateDefinition::builder(State::Pending).build())
        .unwrap()
        .state(StateDefinition::builder(State::Complete).build())
        .unwrap()
        .transition(
            Transition::builder(2)
                .from(State::Pending)
                .on(Input::Finish)
                .to(State::Complete)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut destination = destination_definition.instantiate(Instance(2), ()).unwrap();
    let source_address = InstanceAddress::<OrderIds>::new(5, Instance(1));
    let destination_address = InstanceAddress::<OrderIds>::new(5, Instance(2));
    let registry = InstanceRegistry::<OrderIds>::builder()
        .instance(source_address.clone(), &source)
        .unwrap()
        .instance(destination_address.clone(), &destination)
        .unwrap()
        .build();
    let capability = Capability::<(), (), OrderIds>::builder(7)
        .evaluate(|state, _, _| {
            if *state == State::Pending {
                CapabilityStatus::Available
            } else {
                CapabilityStatus::Unavailable
            }
        })
        .build()
        .unwrap();
    let snapshot = CapabilitySnapshot::<OrderIds>::builder()
        .current_revision(destination_address.clone(), destination.revision())
        .unwrap()
        .report(
            destination_address.clone(),
            destination.evaluate_capability(&capability, &()).unwrap(),
        )
        .unwrap()
        .build();
    let composition = Composition::<u8, OrderIds>::builder(registry.clone(), registry)
        .route(
            Route::builder(source_address.clone(), Input::Finish)
                .to(destination_address)
                .as_event(Input::Finish)
                .requires(7)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let DispatchOutcome::Transitioned { outputs, .. } =
        source.dispatch(Event::new(Input::Finish, ()), &()).unwrap()
    else {
        panic!("expected a transition");
    };
    let items = composition
        .route_batch(&source_address, outputs, &snapshot)
        .unwrap()
        .into_items();
    let RoutedItem::Routed { event, .. } = items.into_iter().next().unwrap() else {
        panic!("expected typed routing");
    };
    assert_eq!(event.id(), &Input::Finish);
    assert_eq!(*event.payload(), 23);
    assert!(destination.dispatch(event, &()).unwrap().is_transitioned());
    assert_eq!(destination.state(), &State::Complete);
}

#[test]
fn non_copy_instance_and_numeric_operation_ids_correlate_retries() {
    let mut waits = RetryWaits::<(), OrderIds>::builder(Instance(9))
        .capacity(1)
        .build()
        .unwrap();
    let deadline = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let intent = waits.schedule(4, deadline, ()).unwrap();
    let correlation = intent;
    assert_eq!(correlation.instance_id(), Instance(9));
    assert_eq!(correlation.operation(), &4);
    assert!(waits.consume(&correlation, deadline).is_some());
    assert!(waits.consume(&correlation, deadline).is_none());
}

#[test]
fn scenario_reports_retain_typed_identities() {
    let definition = source_definition();
    let limits = ScenarioLimits::builder().build().unwrap();
    let scenarios = ScenarioSet::<(), (), OrderIds>::builder(limits)
        .scenario(3, [(Event::new(Input::Finish, ()), ())])
        .unwrap()
        .build()
        .unwrap();
    let assertion = ScenarioAssertion::<OrderIds>::builder(4)
        .evaluate(|observation| *observation.after() == State::Pending)
        .build()
        .unwrap();
    let report =
        ScenarioReplay::run(&definition, Instance(10), (), scenarios, [assertion]).unwrap();
    let ScenarioResult::Counterexample(trace) = report.result() else {
        panic!("expected typed counterexample");
    };
    assert_eq!(trace.scenario(), &3);
    assert_eq!(trace.assertion(), &4);
    assert_eq!(trace.steps()[0].input_event(), &Input::Finish);
    assert_eq!(trace.steps()[0].after(), &State::Complete);
}

#[path = "typed_identifiers/cross_family.rs"]
mod cross_family;
