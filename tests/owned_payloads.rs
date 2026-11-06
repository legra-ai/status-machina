//! Ownership regressions for non-Clone intent payloads and routed events.

use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use state_machine::{
    ActionId,
    CapabilityId,
    CapabilitySnapshot,
    Composition,
    DispatchOutcome,
    Effect,
    EffectId,
    Event,
    EventId,
    InstanceAddress,
    InstanceRegistry,
    LocalAction,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    OutputEvent,
    OutputEventIntent,
    Route,
    RoutedItem,
    RoutingScope,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

#[derive(Debug)]
struct Payload {
    value: usize,
    drops: Arc<AtomicUsize>,
}

impl Payload {
    fn new(value: usize, drops: &Arc<AtomicUsize>) -> Self {
        Self {
            value,
            drops: Arc::clone(drops),
        }
    }

    fn execute(self) -> usize {
        self.value
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

struct Fixture {
    source: MachineDefinition<(), (), (), Payload, Payload>,
    destination: MachineDefinition<usize, Payload, ()>,
    drops: Arc<AtomicUsize>,
}

impl Fixture {
    fn new() -> Self {
        let drops = Arc::new(AtomicUsize::new(0));
        let effect_drops = Arc::clone(&drops);
        let output_drops = Arc::clone(&drops);
        let idle = StateId::try_new("idle").unwrap();
        let effect = Effect::builder(EffectId::try_new("execute").unwrap())
            .describe(move |_: &(), _: &(), _: &()| Payload::new(17, &effect_drops))
            .build()
            .unwrap();
        let output = OutputEvent::builder(EventId::try_new("notice").unwrap())
            .describe(move |_: &(), _: &(), _: &()| Payload::new(23, &output_drops))
            .build()
            .unwrap();
        let transition = Transition::builder_with_effects_and_output_events(
            TransitionId::try_new("send").unwrap(),
        )
        .from(idle.clone())
        .on(EventId::try_new("send").unwrap())
        .to(idle.clone())
        .effect(effect)
        .unwrap()
        .output_event(output)
        .unwrap()
        .build()
        .unwrap();
        let source = MachineDefinition::builder_with_effects_and_output_events(
            MachineId::try_new("source").unwrap(),
        )
        .initial(idle.clone())
        .state(StateDefinition::builder(idle.clone()).build())
        .unwrap()
        .transition(transition)
        .unwrap()
        .build()
        .unwrap();
        let receive = Transition::builder(TransitionId::try_new("receive").unwrap())
            .from(idle.clone())
            .on(EventId::try_new("receive").unwrap())
            .to(idle.clone())
            .action(
                LocalAction::builder(ActionId::try_new("record").unwrap())
                    .apply(|data: &mut usize, payload: &Payload, _: &()| {
                        *data = payload.value;
                        Ok(())
                    })
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
        let destination = MachineDefinition::builder(MachineId::try_new("destination").unwrap())
            .initial(idle.clone())
            .state(StateDefinition::builder(idle).build())
            .unwrap()
            .transition(receive)
            .unwrap()
            .build()
            .unwrap();
        Self {
            source,
            destination,
            drops,
        }
    }

    fn address(value: u128) -> InstanceAddress {
        InstanceAddress::new(
            RoutingScope::try_new("scope").unwrap(),
            MachineInstanceId::try_new(uuid::Uuid::from_u128(
                0x0000_0000_0000_7000_8000_0000_0000_0000 | value,
            ))
            .unwrap(),
        )
    }

    fn emit(&self) -> DispatchOutcome<Payload, Payload> {
        self.source
            .instantiate(Self::address(1).instance_id(), ())
            .unwrap()
            .dispatch(Event::new(EventId::try_new("send").unwrap(), ()), &())
            .unwrap()
    }

    fn output(&self) -> OutputEventIntent<Payload> {
        let DispatchOutcome::Transitioned {
            mut outputs,
            effects,
            ..
        } = self.emit()
        else {
            panic!("expected a transition");
        };
        drop(effects);
        outputs.pop().unwrap()
    }

    fn composition(&self, route: Option<Route>) -> Composition<Payload> {
        let source = self
            .source
            .instantiate(Self::address(1).instance_id(), ())
            .unwrap();
        let destination = self
            .destination
            .instantiate(Self::address(2).instance_id(), 0)
            .unwrap();
        let registry = InstanceRegistry::builder()
            .instance(Self::address(1), &source)
            .unwrap()
            .instance(Self::address(2), &destination)
            .unwrap()
            .build();
        let mut builder = Composition::builder(registry.clone(), registry)
            .limits(1, 1)
            .unwrap();
        if let Some(route) = route {
            builder = builder.route(route).unwrap();
        }
        builder.build().unwrap()
    }

    fn route(capability: Option<CapabilityId>) -> Route {
        let mut route = Route::builder(Self::address(1), EventId::try_new("notice").unwrap())
            .to(Self::address(2))
            .as_event(EventId::try_new("receive").unwrap());
        if let Some(capability) = capability {
            route = route.requires(capability);
        }
        route.build().unwrap()
    }
}

#[test]
fn non_clone_effect_and_output_payloads_move_into_caller_execution() {
    let fixture = Fixture::new();
    let DispatchOutcome::Transitioned {
        mut effects,
        mut outputs,
        ..
    } = fixture.emit()
    else {
        panic!("expected a transition");
    };
    let (effect_id, effect) = effects.pop().unwrap().into_parts();
    let (event_id, output) = outputs.pop().unwrap().into_parts();
    assert_eq!(effect_id.as_str(), "execute");
    assert_eq!(event_id.as_str(), "notice");
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 0);
    assert_eq!(effect.execute(), 17);
    assert_eq!(output.execute(), 23);
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 2);
}

#[test]
fn non_clone_routed_event_moves_directly_into_destination_dispatch() {
    let fixture = Fixture::new();
    let composition = fixture.composition(Some(Fixture::route(None)));
    let output = fixture.output();
    let original_payload = std::ptr::from_ref(output.payload().drops.as_ref());
    let batch = composition
        .route_batch(
            &Fixture::address(1),
            vec![output],
            &CapabilitySnapshot::empty(),
        )
        .unwrap();
    let mut destination = fixture
        .destination
        .instantiate(Fixture::address(2).instance_id(), 0)
        .unwrap();
    assert_eq!(destination.revision(), 0);
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 1);
    let mut items = batch.into_items().into_iter();
    let RoutedItem::Routed {
        event,
        destination: address,
        ..
    } = items.next().unwrap()
    else {
        panic!("expected a routed event");
    };
    assert!(items.next().is_none());
    assert_eq!(address, Fixture::address(2));
    assert_eq!(event.id().as_str(), "receive");
    assert_eq!(
        std::ptr::from_ref(event.payload().drops.as_ref()),
        original_payload
    );
    assert!(destination.dispatch(event, &()).unwrap().is_transitioned());
    assert_eq!(*destination.data(), 23);
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 2);
}

#[test]
fn deferred_and_unmatched_outputs_remain_owned_and_can_be_rerouted() {
    let fixture = Fixture::new();
    let deferred = fixture.composition(Some(Fixture::route(Some(
        CapabilityId::try_new("ready").unwrap(),
    ))));
    let batch = deferred
        .route_batch(
            &Fixture::address(1),
            vec![fixture.output()],
            &CapabilitySnapshot::empty(),
        )
        .unwrap();
    let RoutedItem::Deferred { intent, .. } = batch.into_items().pop().unwrap() else {
        panic!("expected a deferred event");
    };
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 1);
    let unmatched = fixture
        .composition(None)
        .route_batch(
            &Fixture::address(1),
            vec![intent],
            &CapabilitySnapshot::empty(),
        )
        .unwrap();
    let RoutedItem::Unmatched { intent, .. } = unmatched.into_items().pop().unwrap() else {
        panic!("expected an unmatched event");
    };
    let routed = fixture
        .composition(Some(Fixture::route(None)))
        .route_batch(
            &Fixture::address(1),
            vec![intent],
            &CapabilitySnapshot::empty(),
        )
        .unwrap();
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 1);
    drop(routed.into_items());
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 2);
}

#[test]
fn rejected_batches_return_non_clone_payloads_without_dropping_them() {
    let fixture = Fixture::new();
    let composition = fixture.composition(Some(Fixture::route(None)));
    let unknown = composition
        .route_batch(
            &Fixture::address(3),
            vec![fixture.output()],
            &CapabilitySnapshot::empty(),
        )
        .unwrap_err();
    let mut outputs = unknown.into_outputs();
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 1);
    outputs.push(fixture.output());
    let oversized = composition
        .route_batch(&Fixture::address(1), outputs, &CapabilitySnapshot::empty())
        .unwrap_err();
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 2);
    let outputs = oversized.into_outputs();
    assert_eq!(outputs.len(), 2);
    for output in outputs {
        let (id, payload) = output.into_parts();
        assert_eq!(id.as_str(), "notice");
        assert_eq!(payload.execute(), 23);
    }
    assert_eq!(fixture.drops.load(Ordering::SeqCst), 4);
}
