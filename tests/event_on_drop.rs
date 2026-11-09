//! Scope-exit delivery preserves caller ownership and checked dispatch.

use std::cell::{
    Cell,
    RefCell,
};
use std::future::{
    Future,
    pending,
};
use std::panic::{
    AssertUnwindSafe,
    catch_unwind,
};
use std::task::{
    Context,
    Poll,
    Waker,
};

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

fn definition() -> MachineDefinition<(), (), ()> {
    let active = StateId::try_new("active").unwrap();
    let abandoned = StateId::try_new("abandoned").unwrap();
    MachineDefinition::builder(MachineId::try_new("scope").unwrap())
        .initial(active.clone())
        .state(StateDefinition::builder(active.clone()).build())
        .unwrap()
        .state(
            StateDefinition::builder(abandoned.clone())
                .terminal()
                .build(),
        )
        .unwrap()
        .transition(
            Transition::builder(TransitionId::try_new("abandon").unwrap())
                .from(active)
                .on(EventId::try_new("abandon").unwrap())
                .to(abandoned)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn unfinished_scope_dispatches_through_the_checked_kernel() {
    let definition = definition();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            (),
        )
        .unwrap();
    {
        let _guard = EventOnDrop::new(
            Event::new(EventId::try_new("abandon").unwrap(), ()),
            |event| {
                assert!(instance.dispatch(event, &()).unwrap().is_transitioned());
            },
        );
    }
    assert_eq!(instance.state(), &StateId::try_new("abandoned").unwrap());
    assert_eq!(instance.revision(), 1);
}

#[test]
fn invalid_cleanup_event_cannot_assign_a_state() {
    let definition = definition();
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            (),
        )
        .unwrap();
    {
        let _guard = EventOnDrop::new(
            Event::new(EventId::try_new("unknown").unwrap(), ()),
            |event| {
                assert!(!instance.dispatch(event, &()).unwrap().is_transitioned());
            },
        );
    }
    assert_eq!(instance.state(), &StateId::try_new("active").unwrap());
    assert_eq!(instance.revision(), 0);
}

#[test]
fn explicit_completion_disarms_once() {
    let calls = Cell::new(0);
    let mut guard = EventOnDrop::new((), |()| calls.set(calls.get() + 1));
    guard.disarm();
    guard.disarm();
    assert!(guard.event_mut().is_none());
    drop(guard);
    assert_eq!(calls.get(), 0);
}

#[test]
fn early_return_and_unwinding_deliver_once() {
    fn leave_early(calls: &Cell<usize>) -> Result<(), ()> {
        let _guard = EventOnDrop::new((), |()| calls.set(calls.get() + 1));
        Err(())?;
        Ok(())
    }
    let calls = Cell::new(0);
    assert!(leave_early(&calls).is_err());
    assert_eq!(calls.get(), 1);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = EventOnDrop::new((), |()| calls.set(calls.get() + 1));
        panic!("operation failed");
    }));
    assert!(result.is_err());
    assert_eq!(calls.get(), 2);
}

#[test]
fn dropping_a_polled_future_reports_abandonment() {
    let calls = Cell::new(0);
    let mut operation = Box::pin(async {
        let _guard = EventOnDrop::new((), |()| calls.set(calls.get() + 1));
        pending::<()>().await;
    });
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(operation.as_mut().poll(&mut context), Poll::Pending);
    drop(operation);
    assert_eq!(calls.get(), 1);
    // A future never polled has not entered its operation scope.
    let operation = Box::pin(async {
        let _guard = EventOnDrop::new((), |()| calls.set(calls.get() + 1));
    });
    drop(operation);
    assert_eq!(calls.get(), 1);
}

#[test]
fn caller_can_refresh_correlation_without_rearming_a_completed_guard() {
    let observed = Cell::new(0);
    let mut guard = EventOnDrop::new(3, |revision| observed.set(revision));
    *guard.event_mut().unwrap() = 4;
    drop(guard);
    assert_eq!(observed.get(), 4);
}

#[test]
fn obsolete_drop_cannot_change_a_replacement_owner() {
    let owner = Cell::new((1, false));
    let guard = EventOnDrop::new(1, |generation| {
        if owner.get().0 == generation {
            owner.set((generation, true));
        }
    });
    owner.set((2, false));
    drop(guard);
    assert_eq!(owner.get(), (2, false));
}

#[test]
fn move_only_event_and_handler_are_consumed_once() {
    struct Payload<'a>(&'a Cell<usize>);
    impl Drop for Payload<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Cell::new(0);
    let received = RefCell::new(None);
    let captured = Payload(&drops);
    {
        let _guard = EventOnDrop::new(Payload(&drops), |event| {
            drop(captured);
            *received.borrow_mut() = Some(event);
        });
    }
    assert_eq!(drops.get(), 1);
    drop(received);
    assert_eq!(drops.get(), 2);
    let mut guard = EventOnDrop::new(Payload(&drops), drop);
    guard.disarm();
    drop(guard);
    assert_eq!(drops.get(), 3);
}

#[test]
fn handler_failure_is_not_swallowed() {
    let result = catch_unwind(|| {
        let guard = EventOnDrop::new((), |()| panic!("caller cleanup invariant"));
        drop(guard);
    });
    assert!(result.is_err());
}
