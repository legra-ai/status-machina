use status_machina::{
    ActionId,
    Composition,
    Effect,
    EffectId,
    Event,
    EventId,
    InstanceAddress,
    InstanceRegistry,
    LocalAction,
    MachineDefinition,
    MachineId,
    MachineInstance,
    MachineInstanceId,
    OutputEvent,
    Route,
    RoutingScope,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

#[derive(Debug)]
pub(crate) struct Message(pub(crate) u32);
pub(crate) fn id(number: u128) -> MachineInstanceId {
    MachineInstanceId::try_new(uuid::Uuid::from_u128(0x70008000000000000000 | number)).unwrap()
}
pub(crate) fn address(number: u128) -> InstanceAddress {
    InstanceAddress::new(RoutingScope::try_new("composition").unwrap(), id(number))
}
pub(crate) fn event(value: u32) -> Event<Message> {
    Event::new(EventId::try_new("tick").unwrap(), Message(value))
}
pub(crate) fn definition() -> MachineDefinition<u32, Message, bool, u32, Message> {
    definition_with_failure(None)
}
pub(crate) fn definition_with_failure(
    fail_at: Option<u32>,
) -> MachineDefinition<u32, Message, bool, u32, Message> {
    definition_with_outputs(fail_at, 1)
}
pub(crate) fn definition_with_outputs(
    fail_at: Option<u32>,
    outputs: u32,
) -> MachineDefinition<u32, Message, bool, u32, Message> {
    let state = StateId::try_new("active").unwrap();
    let mut edge =
        Transition::builder_with_effects_and_output_events(TransitionId::try_new("tick").unwrap())
            .from(state.clone())
            .on(EventId::try_new("tick").unwrap())
            .to(state.clone())
            .action(
                LocalAction::builder(ActionId::try_new("increment").unwrap())
                    .apply(move |data: &mut u32, event: &Message, permitted: &bool| {
                        if !permitted || fail_at == Some(event.0) {
                            return Err("refused facts".into());
                        }
                        *data += 1;
                        Ok(())
                    })
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .effect(
                Effect::builder(EffectId::try_new("work").unwrap())
                    .describe(|_: &u32, event: &Message, _: &bool| event.0)
                    .build()
                    .unwrap(),
            )
            .unwrap();
    for offset in 1..=outputs {
        edge = edge
            .output_event(
                OutputEvent::builder(EventId::try_new(format!("next-{offset}")).unwrap())
                    .describe(move |_: &u32, event: &Message, _: &bool| Message(event.0 + offset))
                    .build()
                    .unwrap(),
            )
            .unwrap();
    }
    let edge = edge.build().unwrap();
    MachineDefinition::builder_with_effects_and_output_events(
        MachineId::try_new("counter").unwrap(),
    )
    .initial(state.clone())
    .state(StateDefinition::builder(state).build())
    .unwrap()
    .transition(edge)
    .unwrap()
    .build()
    .unwrap()
}
pub(crate) fn routes(
    first: &MachineInstance<'_, u32, Message, bool, u32, Message>,
    second: &MachineInstance<'_, u32, Message, bool, u32, Message>,
) -> Composition<Message> {
    let registry = InstanceRegistry::builder()
        .instance(address(1), first)
        .unwrap()
        .instance(address(2), second)
        .unwrap()
        .build();
    Composition::builder(registry.clone(), registry)
        .route(
            Route::builder(address(1), EventId::try_new("next-1").unwrap())
                .to(address(2))
                .as_event(EventId::try_new("tick").unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
        .route(
            Route::builder(address(2), EventId::try_new("next-1").unwrap())
                .to(address(1))
                .as_event(EventId::try_new("tick").unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}
