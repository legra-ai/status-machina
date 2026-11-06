use state_machine::{
    CapabilitySnapshot,
    Composition,
    DispatchOutcome,
    Event,
    EventId,
    InstanceAddress,
    InstanceRegistry,
    MachineDefinition,
    MachineId,
    MachineInstance,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct CycleMessage {
    step: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Facts;

fn instance_id(value: u128) -> MachineInstanceId {
    const UUID_V7_PREFIX: u128 = 0x0000_0000_0000_7000_8000_0000_0000_0000;
    MachineInstanceId::try_new(uuid::Uuid::from_u128(UUID_V7_PREFIX | value)).unwrap()
}

fn address(value: u128) -> InstanceAddress {
    InstanceAddress::new(
        RoutingScope::try_new("cycle-test").unwrap(),
        instance_id(value),
    )
}

fn machine_definition(
    machine_name: &str,
    initial_name: &str,
    active_name: &str,
    initial_event: &str,
    returned_event: &str,
    output_event_name: &str,
) -> MachineDefinition<(), CycleMessage, Facts, (), CycleMessage> {
    let initial = StateId::try_new(initial_name).unwrap();
    let active = StateId::try_new(active_name).unwrap();
    let initial_transition =
        Transition::<(), CycleMessage, Facts, (), CycleMessage>::builder_with_output_events(
            TransitionId::try_new(format!("{machine_name}-initial")).unwrap(),
        )
        .from(initial.clone())
        .on(EventId::try_new(initial_event).unwrap())
        .to(active.clone())
        .output_event(output_event(output_event_name))
        .unwrap()
        .build()
        .unwrap();
    let return_transition =
        Transition::<(), CycleMessage, Facts, (), CycleMessage>::builder_with_output_events(
            TransitionId::try_new(format!("{machine_name}-return")).unwrap(),
        )
        .from(active.clone())
        .on(EventId::try_new(returned_event).unwrap())
        .to(active.clone())
        .output_event(output_event(output_event_name))
        .unwrap()
        .build()
        .unwrap();

    MachineDefinition::builder_with_output_events(MachineId::try_new(machine_name).unwrap())
        .initial(initial.clone())
        .state(StateDefinition::builder(initial).build())
        .unwrap()
        .state(StateDefinition::builder(active).build())
        .unwrap()
        .transition(initial_transition)
        .unwrap()
        .transition(return_transition)
        .unwrap()
        .build()
        .unwrap()
}

fn output_event(name: &str) -> OutputEvent<(), CycleMessage, Facts, CycleMessage> {
    OutputEvent::builder(EventId::try_new(name).unwrap())
        .describe(
            |_data: &(), event: &CycleMessage, _facts: &Facts| CycleMessage {
                step: event.step + 1,
            },
        )
        .build()
        .unwrap()
}

fn dispatch(
    instance: &mut MachineInstance<'_, (), CycleMessage, Facts, (), CycleMessage>,
    event_id: &str,
    step: u8,
) -> Vec<state_machine::OutputEventIntent<CycleMessage>> {
    let DispatchOutcome::Transitioned { outputs, .. } = instance
        .dispatch(
            Event::new(EventId::try_new(event_id).unwrap(), CycleMessage { step }),
            &Facts,
        )
        .unwrap()
    else {
        panic!("expected accepted transition");
    };
    outputs
}

#[test]
fn cyclic_routing_advances_only_when_the_caller_dispatches_and_stops_at_its_bound() {
    const MAX_STEPS: usize = 4;
    let address_a = address(1);
    let address_b = address(2);
    let definition_a = machine_definition(
        "machine-a",
        "a-start",
        "a-active",
        "start-a",
        "receive-b",
        "a-to-b",
    );
    let definition_b = machine_definition(
        "machine-b",
        "b-waiting",
        "b-active",
        "receive-a",
        "receive-a",
        "b-to-a",
    );
    let mut instance_a = definition_a
        .instantiate(address_a.instance_id(), ())
        .unwrap();
    let mut instance_b = definition_b
        .instantiate(address_b.instance_id(), ())
        .unwrap();

    let registry = InstanceRegistry::builder()
        .instance(address_a.clone(), &instance_a)
        .unwrap()
        .instance(address_b.clone(), &instance_b)
        .unwrap()
        .build();
    let composition = Composition::<CycleMessage>::builder(registry.clone(), registry)
        .route(
            Route::builder(address_a.clone(), EventId::try_new("a-to-b").unwrap())
                .to(address_b.clone())
                .as_event(EventId::try_new("receive-a").unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
        .route(
            Route::builder(address_b.clone(), EventId::try_new("b-to-a").unwrap())
                .to(address_a.clone())
                .as_event(EventId::try_new("receive-b").unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();

    let mut source = address_a.clone();
    let mut outputs = dispatch(&mut instance_a, "start-a", 0);
    for step in 0..MAX_STEPS {
        let prior_a = instance_a.state().clone();
        let prior_b = instance_b.state().clone();
        let batch = composition
            .route_batch(&source, outputs, &CapabilitySnapshot::empty())
            .unwrap();
        let [
            RoutedItem::Routed {
                destination, event, ..
            },
        ] = batch.items()
        else {
            panic!("expected one routed event at caller step {step}");
        };
        assert_eq!(instance_a.state(), &prior_a);
        assert_eq!(instance_b.state(), &prior_b);

        let (target, next_source) = if destination == &address_a {
            (&mut instance_a, address_a.clone())
        } else {
            assert_eq!(destination, &address_b);
            (&mut instance_b, address_b.clone())
        };
        let DispatchOutcome::Transitioned {
            outputs: next_outputs,
            ..
        } = target.dispatch(event.clone(), &Facts).unwrap()
        else {
            panic!("caller dispatch did not accept the routed event");
        };
        assert_eq!(event.payload().step, step as u8 + 1);
        source = next_source;
        outputs = next_outputs;
    }

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].payload().step, MAX_STEPS as u8 + 1);
    assert_eq!(source, address_a);
    assert_eq!(instance_a.state().as_str(), "a-active");
    assert_eq!(instance_b.state().as_str(), "b-active");
}
