//! Thread-bound native definitions remain owned without unsafe Send claims.
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use status_machina::{
    Event,
    EventId,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

#[test]
fn shared_definition_can_own_thread_bound_data_without_an_unsafe_send_claim() {
    let ready = StateId::try_new("ready").unwrap();
    let done = StateId::try_new("done").unwrap();
    let event = EventId::try_new("go").unwrap();
    let definition = Arc::new(
        MachineDefinition::<LocalData, (), ()>::builder(MachineId::try_new("test").unwrap())
            .initial(ready.clone())
            .state(StateDefinition::builder(ready.clone()).build())
            .unwrap()
            .state(StateDefinition::builder(done.clone()).build())
            .unwrap()
            .transition(
                Transition::builder(TransitionId::try_new("go").unwrap())
                    .from(ready)
                    .to(done.clone())
                    .on(event.clone())
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap(),
    );
    let mut instance = Arc::clone(&definition)
        .instantiate_shared(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            LocalData(Rc::new(Cell::new(7))),
        )
        .unwrap();
    drop(definition);
    assert!(
        instance
            .dispatch(Event::new(event, ()), &())
            .unwrap()
            .is_transitioned()
    );
    assert_eq!(instance.state(), &done);
}

struct LocalData(Rc<Cell<u64>>);
impl status_machina::FallibleCandidateData for LocalData {
    fn try_snapshot(&self) -> Result<Self, status_machina::EvaluationError> {
        Ok(Self(Rc::new(Cell::new(self.0.get()))))
    }
}
