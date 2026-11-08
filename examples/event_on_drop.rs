//! A surviving owner observes abandonment and explicit completion.
use std::cell::RefCell;

use status_machina::{
    Event,
    EventId,
    EventOnDrop,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

fn main() {
    let running = StateId::try_new("running").unwrap();
    let completed = StateId::try_new("completed").unwrap();
    let abandoned = StateId::try_new("abandoned").unwrap();
    let finish = EventId::try_new("finish").unwrap();
    let abandon = EventId::try_new("abandon").unwrap();
    let mut builder =
        MachineDefinition::<(), (), ()>::builder(MachineId::try_new("operation").unwrap())
            .initial(running.clone())
            .state(StateDefinition::builder(running.clone()).build())
            .unwrap()
            .state(
                StateDefinition::builder(completed.clone())
                    .terminal()
                    .build(),
            )
            .unwrap()
            .state(
                StateDefinition::builder(abandoned.clone())
                    .terminal()
                    .build(),
            )
            .unwrap();
    for (name, event, target) in [
        ("finish", finish.clone(), completed.clone()),
        ("abandon", abandon.clone(), abandoned.clone()),
    ] {
        builder = builder
            .transition(
                Transition::builder(TransitionId::try_new(name).unwrap())
                    .from(running.clone())
                    .on(event)
                    .to(target)
                    .build()
                    .unwrap(),
            )
            .unwrap();
    }
    let definition = builder.build().unwrap();
    let owner = RefCell::new(
        definition
            .instantiate(
                MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
                (),
            )
            .unwrap(),
    );
    {
        let _scope = EventOnDrop::new(Event::new(abandon.clone(), ()), |event| {
            // No owner borrow or lock is held when this callback runs.
            assert!(
                owner
                    .borrow_mut()
                    .dispatch(event, &())
                    .unwrap()
                    .is_transitioned()
            );
        });
        // An early return, panic unwind or cancellation would also report
        // abandonment.
    }
    assert_eq!(owner.borrow().state(), &abandoned);

    *owner.borrow_mut() = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            (),
        )
        .unwrap();
    {
        let mut scope = EventOnDrop::new(Event::new(abandon, ()), |event| {
            assert!(
                owner
                    .borrow_mut()
                    .dispatch(event, &())
                    .unwrap()
                    .is_transitioned()
            );
        });
        assert!(
            owner
                .borrow_mut()
                .dispatch(Event::new(finish, ()), &())
                .unwrap()
                .is_transitioned()
        );
        scope.disarm(); // Only after the actual completion was accepted.
    }
    assert_eq!(owner.borrow().state(), &completed);
}
