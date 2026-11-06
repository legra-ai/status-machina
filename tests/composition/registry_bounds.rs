use state_machine::InstanceRegistryBuildError;

use super::*;

fn large_event_definition() -> MachineDefinition<(), (), (), (), ()> {
    let state = StateId::try_new("active").unwrap();
    let mut definition = MachineDefinition::<(), (), (), (), ()>::builder_with_output_events(
        MachineId::try_new("many-events").unwrap(),
    )
    .initial(state.clone())
    .state(StateDefinition::builder(state.clone()).build())
    .unwrap();

    for transition_index in 0..64 {
        let transition_id =
            TransitionId::try_new(format!("transition-{transition_index}")).unwrap();
        let input_id = EventId::try_new(format!("input-{transition_index}")).unwrap();
        let mut transition =
            Transition::<(), (), (), (), ()>::builder_with_output_events(transition_id)
                .from(state.clone())
                .on(input_id)
                .to(state.clone());
        for output_index in 0..16 {
            let output_id =
                EventId::try_new(format!("output-{transition_index}-{output_index}")).unwrap();
            let output = OutputEvent::builder(output_id)
                .describe(|_data: &(), _input: &(), _facts: &()| ())
                .build()
                .unwrap();
            transition = transition.output_event(output).unwrap();
        }
        definition = definition.transition(transition.build().unwrap()).unwrap();
    }

    definition.build().unwrap()
}

#[test]
fn registry_bounds_the_aggregate_copied_event_id_inventory() {
    let definition = large_event_definition();
    let mut registry = InstanceRegistry::builder();

    for value in 1..=60 {
        let address = address("event-inventory", value);
        let instance = definition.instantiate(address.instance_id(), ()).unwrap();
        registry = registry.instance(address, &instance).unwrap();
    }

    let address = address("event-inventory", 61);
    let instance = definition.instantiate(address.instance_id(), ()).unwrap();
    assert!(matches!(
        registry.instance(address, &instance),
        Err(InstanceRegistryBuildError::AggregateEventLimitExceeded { limit: 65_536 })
    ));
}
