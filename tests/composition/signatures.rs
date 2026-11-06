use state_machine::{
    Composition,
    CompositionBuildError,
    EventId,
    InstanceAddress,
    InstanceRegistry,
    InstanceRegistryBuildError,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    OutputEvent,
    Route,
    RouteBuildError,
    RoutingScope,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Start;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Notice;

#[derive(Clone, Debug, Eq, PartialEq)]
struct OtherNotice;

fn instance_id(value: u128) -> MachineInstanceId {
    const UUID_V7_PREFIX: u128 = 0x0000_0000_0000_7000_8000_0000_0000_0000;
    MachineInstanceId::try_new(uuid::Uuid::from_u128(UUID_V7_PREFIX | value)).unwrap()
}

fn address(scope: &str, value: u128) -> InstanceAddress {
    InstanceAddress::new(RoutingScope::try_new(scope).unwrap(), instance_id(value))
}

fn source_definition() -> MachineDefinition<(), Start, (), (), Notice> {
    let initial = StateId::try_new("initial").unwrap();
    let sent = StateId::try_new("sent").unwrap();
    let output = OutputEvent::builder(EventId::try_new("notify").unwrap())
        .describe(|_data: &(), _event: &Start, _facts: &()| Notice)
        .build()
        .unwrap();
    let transition = Transition::<(), Start, (), (), Notice>::builder_with_output_events(
        TransitionId::try_new("send-notification").unwrap(),
    )
    .from(initial.clone())
    .on(EventId::try_new("start").unwrap())
    .to(sent.clone())
    .output_event(output)
    .unwrap()
    .build()
    .unwrap();
    MachineDefinition::<(), Start, (), (), Notice>::builder_with_output_events(
        MachineId::try_new("source").unwrap(),
    )
    .initial(initial.clone())
    .state(StateDefinition::builder(initial).build())
    .unwrap()
    .state(StateDefinition::builder(sent).build())
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap()
}

fn notice_destination() -> MachineDefinition<(), Notice, ()> {
    destination_definition::<Notice>("notice-destination")
}

fn other_destination() -> MachineDefinition<(), OtherNotice, ()> {
    destination_definition::<OtherNotice>("other-destination")
}

fn destination_definition<E>(name: &str) -> MachineDefinition<(), E, ()> {
    let initial = StateId::try_new("initial").unwrap();
    let received = StateId::try_new("received").unwrap();
    let transition = Transition::builder(TransitionId::try_new("receive").unwrap())
        .from(initial.clone())
        .on(EventId::try_new("receive-notification").unwrap())
        .to(received.clone())
        .build()
        .unwrap();
    MachineDefinition::builder(MachineId::try_new(name).unwrap())
        .initial(initial.clone())
        .state(StateDefinition::builder(initial).build())
        .unwrap()
        .state(StateDefinition::builder(received).build())
        .unwrap()
        .transition(transition)
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn registry_requires_the_address_to_match_the_registered_instance() {
    let definition = source_definition();
    let actual_address = address("tenant", 1);
    let wrong_address = address("tenant", 2);
    let instance = definition
        .instantiate(actual_address.instance_id(), ())
        .unwrap();

    assert!(matches!(
        InstanceRegistry::builder().instance(wrong_address, &instance),
        Err(InstanceRegistryBuildError::AddressMismatch)
    ));
}

#[test]
fn route_requires_a_destination_input_event() {
    let error = Route::builder(address("tenant", 3), EventId::try_new("notify").unwrap())
        .to(address("tenant", 4))
        .build()
        .err();

    assert_eq!(error, Some(RouteBuildError::MissingInputEvent));
}

#[test]
fn composition_build_rejects_undeclared_source_and_destination_event_ids() {
    let source = address("tenant", 5);
    let destination = address("tenant", 6);
    let source_definition = source_definition();
    let source_instance = source_definition
        .instantiate(source.instance_id(), ())
        .unwrap();
    let destination_definition = notice_destination();
    let destination_instance = destination_definition
        .instantiate(destination.instance_id(), ())
        .unwrap();
    let registry = InstanceRegistry::builder()
        .instance(source.clone(), &source_instance)
        .unwrap()
        .instance(destination.clone(), &destination_instance)
        .unwrap()
        .build();

    let undeclared_source = Composition::<Notice>::builder(registry.clone(), registry)
        .route(
            Route::builder(source.clone(), EventId::try_new("other-output").unwrap())
                .to(destination.clone())
                .as_event(EventId::try_new("receive-notification").unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
        .build();
    assert!(matches!(
        undeclared_source,
        Err(CompositionBuildError::UndeclaredSourceEvent)
    ));

    let registry = InstanceRegistry::builder()
        .instance(source.clone(), &source_instance)
        .unwrap()
        .instance(destination.clone(), &destination_instance)
        .unwrap()
        .build();
    let undeclared_destination = Composition::<Notice>::builder(registry.clone(), registry)
        .route(
            Route::builder(source, EventId::try_new("notify").unwrap())
                .to(destination)
                .as_event(EventId::try_new("other-input").unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
        .build();
    assert!(matches!(
        undeclared_destination,
        Err(CompositionBuildError::UndeclaredDestinationEvent)
    ));
}

#[test]
fn composition_build_rejects_payload_types_incompatible_with_destination() {
    let source = address("tenant", 7);
    let destination = address("tenant", 8);
    let source_definition = source_definition();
    let source_instance = source_definition
        .instantiate(source.instance_id(), ())
        .unwrap();
    let destination_definition = other_destination();
    let destination_instance = destination_definition
        .instantiate(destination.instance_id(), ())
        .unwrap();
    let registry = InstanceRegistry::builder()
        .instance(source.clone(), &source_instance)
        .unwrap()
        .instance(destination.clone(), &destination_instance)
        .unwrap()
        .build();
    let composition = Composition::<Notice>::builder(registry.clone(), registry)
        .route(
            Route::builder(source, EventId::try_new("notify").unwrap())
                .to(destination)
                .as_event(EventId::try_new("receive-notification").unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
        .build();

    assert!(matches!(
        composition,
        Err(CompositionBuildError::DestinationPayloadMismatch)
    ));
}

#[test]
fn composition_build_rejects_payload_types_incompatible_with_source() {
    let source = address("tenant", 9);
    let destination = address("tenant", 10);
    let source_definition = source_definition();
    let source_instance = source_definition
        .instantiate(source.instance_id(), ())
        .unwrap();
    let destination_definition = other_destination();
    let destination_instance = destination_definition
        .instantiate(destination.instance_id(), ())
        .unwrap();
    let registry = InstanceRegistry::builder()
        .instance(source.clone(), &source_instance)
        .unwrap()
        .instance(destination.clone(), &destination_instance)
        .unwrap()
        .build();
    let composition = Composition::<OtherNotice>::builder(registry.clone(), registry)
        .route(
            Route::builder(source, EventId::try_new("notify").unwrap())
                .to(destination)
                .as_event(EventId::try_new("receive-notification").unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
        .build();

    assert!(matches!(
        composition,
        Err(CompositionBuildError::SourcePayloadMismatch)
    ));
}
