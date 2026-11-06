//! Public API tests for explicit bounded state-machine composition.

use state_machine::{
    Capability,
    CapabilityId,
    CapabilitySnapshot,
    CapabilityStatus,
    Composition,
    DispatchOutcome,
    Event,
    EventId,
    InstanceAddress,
    InstanceRegistry,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    OutputEvent,
    Route,
    RoutedItem,
    RoutingScope,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

#[path = "composition/cycles.rs"]
mod cycles;
#[path = "composition/registration.rs"]
mod registration;
#[path = "composition/registry_bounds.rs"]
mod registry_bounds;
#[path = "composition/signatures.rs"]
mod signatures;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Begin;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Notification {
    correlation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Facts;

fn instance_id(value: u128) -> MachineInstanceId {
    const UUID_V7_PREFIX: u128 = 0x0000_0000_0000_7000_8000_0000_0000_0000;
    MachineInstanceId::try_new(uuid::Uuid::from_u128(UUID_V7_PREFIX | value)).unwrap()
}

fn address(scope: &str, value: u128) -> InstanceAddress {
    InstanceAddress::new(RoutingScope::try_new(scope).unwrap(), instance_id(value))
}

fn source_definition() -> MachineDefinition<(), Begin, Facts, (), Notification> {
    let idle = StateId::try_new("idle").unwrap();
    let sent = StateId::try_new("sent").unwrap();
    let begin = EventId::try_new("begin").unwrap();
    let output = OutputEvent::builder(EventId::try_new("notification").unwrap())
        .describe(|_data: &(), _event: &Begin, _facts: &Facts| Notification { correlation: 17 })
        .build()
        .unwrap();
    let transition = Transition::<(), Begin, Facts, (), Notification>::builder_with_output_events(
        TransitionId::try_new("send-notification").unwrap(),
    )
    .from(idle.clone())
    .on(begin)
    .to(sent.clone())
    .output_event(output)
    .unwrap()
    .build()
    .unwrap();

    MachineDefinition::<(), Begin, Facts, (), Notification>::builder_with_output_events(
        MachineId::try_new("source").unwrap(),
    )
    .initial(idle.clone())
    .state(StateDefinition::builder(idle).build())
    .unwrap()
    .state(StateDefinition::builder(sent).build())
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap()
}

fn destination_definition() -> MachineDefinition<(), Notification, Facts> {
    let waiting = StateId::try_new("waiting").unwrap();
    let notified = StateId::try_new("notified").unwrap();
    let transition = Transition::builder(TransitionId::try_new("accept-notification").unwrap())
        .from(waiting.clone())
        .on(EventId::try_new("receive-notification").unwrap())
        .to(notified.clone())
        .build()
        .unwrap();

    MachineDefinition::builder(MachineId::try_new("destination").unwrap())
        .initial(waiting.clone())
        .state(StateDefinition::builder(waiting).build())
        .unwrap()
        .state(StateDefinition::builder(notified).build())
        .unwrap()
        .transition(transition)
        .unwrap()
        .build()
        .unwrap()
}

fn source_output_for(
    instance_id: MachineInstanceId,
) -> state_machine::OutputEventIntent<Notification> {
    let definition = source_definition();
    let mut instance = definition.instantiate(instance_id, ()).unwrap();
    let DispatchOutcome::Transitioned { outputs, .. } = instance
        .dispatch(
            Event::new(EventId::try_new("begin").unwrap(), Begin),
            &Facts,
        )
        .unwrap()
    else {
        panic!("source transition did not emit an output event");
    };
    outputs.into_iter().next().unwrap()
}

fn registry(source: &InstanceAddress, destination: &InstanceAddress) -> InstanceRegistry {
    let source_definition = source_definition();
    let source_instance = source_definition
        .instantiate(source.instance_id(), ())
        .unwrap();
    let destination_definition = destination_definition();
    let destination_instance = destination_definition
        .instantiate(destination.instance_id(), ())
        .unwrap();
    InstanceRegistry::builder()
        .instance(source.clone(), &source_instance)
        .unwrap()
        .instance(destination.clone(), &destination_instance)
        .unwrap()
        .build()
}

fn composition(
    source: &InstanceAddress,
    destination: &InstanceAddress,
    required_capability: Option<CapabilityId>,
) -> Composition<Notification> {
    let mut route = Route::builder(source.clone(), EventId::try_new("notification").unwrap())
        .to(destination.clone())
        .as_event(EventId::try_new("receive-notification").unwrap());
    if let Some(capability) = required_capability {
        route = route.requires(capability);
    }
    Composition::builder(registry(source, destination), registry(source, destination))
        .route(route.build().unwrap())
        .unwrap()
        .build()
        .unwrap()
}

fn available_capabilities(
    destination: &InstanceAddress,
    revision: u64,
    status: CapabilityStatus,
) -> CapabilitySnapshot {
    let capability = Capability::builder(CapabilityId::try_new("can-receive").unwrap())
        .evaluate(move |_state: &StateId, _data: &(), _facts: &Facts| status)
        .build()
        .unwrap();
    let definition = destination_definition();
    let instance = definition
        .instantiate(destination.instance_id(), ())
        .unwrap();
    let report = instance.evaluate_capability(&capability, &Facts);

    CapabilitySnapshot::builder()
        .current_revision(destination.clone(), revision)
        .unwrap()
        .report(destination.clone(), report)
        .unwrap()
        .build()
}

#[test]
fn output_is_routed_as_data_and_destination_runs_only_when_caller_dispatches() {
    let source = address("tenant-a", 1);
    let destination = address("tenant-a", 2);
    let composition = composition(&source, &destination, None);
    let destination_definition = destination_definition();
    let mut destination_instance = destination_definition
        .instantiate(destination.instance_id(), ())
        .unwrap();

    let batch = composition
        .route_batch(
            &source,
            vec![source_output_for(source.instance_id())],
            &CapabilitySnapshot::empty(),
        )
        .unwrap();

    assert_eq!(destination_instance.state().as_str(), "waiting");
    let [
        RoutedItem::Routed {
            destination: address,
            event,
            ..
        },
    ] = batch.items()
    else {
        panic!("expected one routed event");
    };
    assert_eq!(address, &destination);
    assert_eq!(event.id().as_str(), "receive-notification");
    assert_eq!(event.payload().correlation, 17);

    let outcome = destination_instance
        .dispatch(event.clone(), &Facts)
        .unwrap();
    assert!(outcome.is_transitioned());
    assert_eq!(destination_instance.state().as_str(), "notified");
}

#[test]
fn missing_routes_and_unavailable_capabilities_retain_the_original_intent() {
    let source = address("tenant-a", 3);
    let destination = address("tenant-a", 4);
    let composition = composition(
        &source,
        &destination,
        Some(CapabilityId::try_new("can-receive").unwrap()),
    );
    let intent = source_output_for(source.instance_id());

    let missing_observation = composition
        .route_batch(&source, vec![intent.clone()], &CapabilitySnapshot::empty())
        .unwrap();
    assert!(matches!(
        &missing_observation.items()[0],
        RoutedItem::Deferred { intent: retained, status: CapabilityStatus::Unknown, .. }
            if retained == &intent
    ));

    let unavailable = available_capabilities(&destination, 0, CapabilityStatus::Unavailable);
    let unavailable_batch = composition
        .route_batch(&source, vec![intent.clone()], &unavailable)
        .unwrap();
    assert!(matches!(
        &unavailable_batch.items()[0],
        RoutedItem::Deferred { intent: retained, status: CapabilityStatus::Unavailable, .. }
            if retained == &intent
    ));

    let unmatched = Composition::<Notification>::builder(
        registry(&source, &destination),
        registry(&source, &destination),
    )
    .build()
    .unwrap()
    .route_batch(&source, vec![intent.clone()], &CapabilitySnapshot::empty())
    .unwrap();
    assert!(matches!(
        &unmatched.items()[0],
        RoutedItem::Unmatched { intent: retained, .. } if retained == &intent
    ));
}

#[test]
fn capability_is_advisory_and_only_current_instance_revision_routes() {
    let source = address("tenant-a", 5);
    let destination = address("tenant-a", 6);
    let composition = composition(
        &source,
        &destination,
        Some(CapabilityId::try_new("can-receive").unwrap()),
    );
    let intent = source_output_for(source.instance_id());
    let available = available_capabilities(&destination, 0, CapabilityStatus::Available);
    assert!(matches!(
        composition
            .route_batch(&source, vec![intent.clone()], &available)
            .unwrap()
            .items()[0],
        RoutedItem::Routed { .. }
    ));

    let stale = available_capabilities(&destination, 1, CapabilityStatus::Available);
    assert!(matches!(
        &composition
            .route_batch(&source, vec![intent.clone()], &stale)
            .unwrap()
            .items()[0],
        RoutedItem::Deferred { intent: retained, status: CapabilityStatus::Unknown, .. }
            if retained == &intent
    ));
}

#[test]
fn composition_rejects_unregistered_addresses_and_oversized_batches_without_consuming_inputs() {
    let source = address("tenant-a", 7);
    let destination = address("tenant-a", 8);
    let other_source = address("tenant-b", 9);
    let composition = composition(&source, &destination, None);
    let intent = source_output_for(source.instance_id());

    let unknown_source = composition
        .route_batch(
            &other_source,
            vec![intent.clone()],
            &CapabilitySnapshot::empty(),
        )
        .unwrap_err();
    assert_eq!(unknown_source.into_outputs(), vec![intent.clone()]);

    let bounded = Composition::<Notification>::builder(
        registry(&source, &destination),
        registry(&source, &destination),
    )
    .limits(1, 1)
    .unwrap()
    .route(
        Route::builder(source.clone(), EventId::try_new("notification").unwrap())
            .to(destination.clone())
            .as_event(EventId::try_new("receive-notification").unwrap())
            .build()
            .unwrap(),
    )
    .unwrap()
    .build()
    .unwrap();
    let oversized = bounded
        .route_batch(
            &source,
            vec![intent.clone(), intent.clone()],
            &CapabilitySnapshot::empty(),
        )
        .unwrap_err();
    assert_eq!(oversized.into_outputs(), vec![intent.clone(), intent]);
}
