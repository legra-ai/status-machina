use status_machina::{
    ActionId,
    CapabilitySnapshot,
    Composition,
    Condition,
    ConditionId,
    Delivery,
    Effect,
    EffectId,
    Event,
    EventId,
    InstanceRegistry,
    LocalAction,
    MachineDefinition,
    MachineId,
    OutputEvent,
    PropagationLimits,
    PropagationStatus,
    Route,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

use super::support::{
    Message,
    address,
    id,
};

fn requester() -> MachineDefinition<bool, Message, bool, u32, Message> {
    let start =
        Transition::builder_with_effects_and_output_events(TransitionId::try_new("start").unwrap())
            .from(state("idle"))
            .on(kind("start"))
            .to(state("waiting"))
            .output_event(
                OutputEvent::builder(kind("request"))
                    .describe(|_: &bool, event: &Message, _: &bool| Message(event.0))
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
    let finish = Transition::builder_with_effects_and_output_events(
        TransitionId::try_new("finish").unwrap(),
    )
    .from(state("waiting"))
    .on(kind("finished"))
    .to(state("done"))
    .action(
        LocalAction::builder(ActionId::try_new("complete").unwrap())
            .apply(|data: &mut bool, _: &Message, _: &bool| {
                *data = true;
                Ok(())
            })
            .build()
            .unwrap(),
    )
    .unwrap()
    .build()
    .unwrap();
    MachineDefinition::builder_with_effects_and_output_events(
        MachineId::try_new("requester").unwrap(),
    )
    .initial(state("idle"))
    .state(StateDefinition::builder(state("idle")).build())
    .unwrap()
    .state(StateDefinition::builder(state("waiting")).build())
    .unwrap()
    .state(StateDefinition::builder(state("done")).terminal().build())
    .unwrap()
    .transition(start)
    .unwrap()
    .transition(finish)
    .unwrap()
    .build()
    .unwrap()
}
fn worker() -> MachineDefinition<u64, Message, bool, u32, Message> {
    let start =
        Transition::builder_with_effects_and_output_events(TransitionId::try_new("start").unwrap())
            .from(state("idle"))
            .on(kind("request"))
            .to(state("waiting"))
            .action(
                LocalAction::builder(ActionId::try_new("operation").unwrap())
                    .apply(|data: &mut u64, event: &Message, _: &bool| {
                        *data = u64::from(event.0);
                        Ok(())
                    })
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .effect(
                Effect::builder(EffectId::try_new("external work").unwrap())
                    .describe(|_: &u64, event: &Message, _: &bool| event.0)
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
    let finish = Transition::builder_with_effects_and_output_events(
        TransitionId::try_new("finish").unwrap(),
    )
    .from(state("waiting"))
    .on(kind("completion"))
    .to(state("done"))
    .when(
        Condition::builder(ConditionId::try_new("current operation authorized").unwrap())
            .evaluate(|data: &u64, event: &Message, authorized: &bool| {
                *authorized && *data == u64::from(event.0)
            })
            .build()
            .unwrap(),
    )
    .output_event(
        OutputEvent::builder(kind("finished"))
            .describe(|_: &u64, event: &Message, _: &bool| Message(event.0))
            .build()
            .unwrap(),
    )
    .unwrap()
    .build()
    .unwrap();
    MachineDefinition::builder_with_effects_and_output_events(MachineId::try_new("worker").unwrap())
        .initial(state("idle"))
        .state(StateDefinition::builder(state("idle")).build())
        .unwrap()
        .state(StateDefinition::builder(state("waiting")).build())
        .unwrap()
        .state(StateDefinition::builder(state("done")).terminal().build())
        .unwrap()
        .transition(start)
        .unwrap()
        .transition(finish)
        .unwrap()
        .build()
        .unwrap()
}
fn state(name: &str) -> StateId {
    StateId::try_new(name).unwrap()
}
fn kind(name: &str) -> EventId {
    EventId::try_new(name).unwrap()
}

#[test]
fn connected_waiting_machines_return_work_then_accept_only_current_completion() {
    let requester_definition = requester();
    let worker_definition = worker();
    let mut requester = requester_definition.instantiate(id(1), false).unwrap();
    let mut worker = worker_definition.instantiate(id(2), 0_u64).unwrap();
    let registry = InstanceRegistry::builder()
        .instance(address(1), &requester)
        .unwrap()
        .instance(address(2), &worker)
        .unwrap()
        .build();
    let composition = Composition::builder(registry.clone(), registry)
        .route(
            Route::builder(address(1), kind("request"))
                .to(address(2))
                .as_event(kind("request"))
                .build()
                .unwrap(),
        )
        .unwrap()
        .route(
            Route::builder(address(2), kind("finished"))
                .to(address(1))
                .as_event(kind("finished"))
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let caps = CapabilitySnapshot::builder().build();
    let limits = PropagationLimits::builder()
        .max_steps(8)
        .max_intents(8)
        .build()
        .unwrap();
    let report = composition
        .bind()
        .instance(address(1), &mut requester)
        .unwrap()
        .instance(address(2), &mut worker)
        .unwrap()
        .build()
        .unwrap()
        .propagate(
            vec![Delivery::new(
                address(1),
                Event::new(kind("start"), Message(42)),
            )],
            &true,
            &caps,
            limits,
        );
    assert_eq!(report.status, PropagationStatus::Quiescent);
    assert_eq!(report.steps.len(), 2);
    assert_eq!(requester.state(), &state("waiting"));
    assert_eq!(worker.state(), &state("waiting"));
    assert!(!requester.data());
    // External execution is just caller code, after all machine borrows end.
    let operation = report
        .effects
        .into_iter()
        .next()
        .unwrap()
        .intent
        .into_parts()
        .1;
    assert_eq!(operation, 42);
    for (completion, authorized) in [(41, true), (42, false), (42, true), (42, true)] {
        let report = composition
            .bind()
            .instance(address(1), &mut requester)
            .unwrap()
            .instance(address(2), &mut worker)
            .unwrap()
            .build()
            .unwrap()
            .propagate(
                vec![Delivery::new(
                    address(2),
                    Event::new(kind("completion"), Message(completion)),
                )],
                &authorized,
                &caps,
                limits,
            );
        assert!(report.effects.is_empty());
        if completion == 41 || !authorized {
            assert!(report.steps[0].outcome.is_refused());
        }
    }
    assert_eq!(worker.revision(), 2);
    assert_eq!(requester.revision(), 2);
    assert!(*requester.data());
}
