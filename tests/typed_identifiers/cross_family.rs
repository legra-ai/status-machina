use state_machine::{
    CompositionBuildError,
    DiagramLabels,
    InspectionLimits,
};

use super::*;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReceiverIds;

impl MachineIds for ReceiverIds {
    type Definition = u32;
    type Instance = u32;
    type State = bool;
    type Event = u16;
    type Transition = u16;
    type Action = u16;
    type Condition = u16;
    type Effect = u16;
    type Disposition = u16;
    type Capability = u16;
    type Operation = u16;
    type Scenario = u16;
    type Assertion = u16;
    type Scope = u16;
    type Behavior = u16;
    type Implementation = u16;
}

fn receiver<E>() -> MachineDefinition<(), E, (), (), (), ReceiverIds> {
    MachineDefinition::builder(99)
        .initial(false)
        .state(StateDefinition::builder(false).build())
        .unwrap()
        .state(StateDefinition::builder(true).build())
        .unwrap()
        .transition(
            Transition::builder(1)
                .from(false)
                .on(50)
                .to(true)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn explicit_route_maps_distinct_identity_families() {
    let definition = source_definition();
    let mut source = definition.instantiate(Instance(1), ()).unwrap();
    let target_definition = receiver::<u8>();
    let mut target = target_definition.instantiate(2, ()).unwrap();
    let source_address = InstanceAddress::<OrderIds>::new(5, Instance(1));
    let target_address = InstanceAddress::<ReceiverIds>::new(6, 2);
    let sources = InstanceRegistry::builder()
        .instance(source_address.clone(), &source)
        .unwrap()
        .build();
    let targets = InstanceRegistry::builder()
        .instance(target_address.clone(), &target)
        .unwrap()
        .build();
    let composition = Composition::<u8, OrderIds, ReceiverIds>::builder(sources, targets)
        .route(
            Route::builder(source_address.clone(), Input::Finish)
                .to(target_address.clone())
                .as_event(50)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let DispatchOutcome::Transitioned { outputs, .. } =
        source.dispatch(Event::new(Input::Finish, ()), &()).unwrap()
    else {
        panic!("expected transition")
    };
    let batch = composition
        .route_batch(
            &source_address,
            outputs,
            &CapabilitySnapshot::builder().build(),
        )
        .unwrap();
    let RoutedItem::Routed {
        destination, event, ..
    } = batch.into_items().pop().unwrap()
    else {
        panic!("expected mapped event")
    };
    assert_eq!(destination, target_address);
    assert_eq!(event.id(), &50);
    assert_eq!(*event.payload(), 23);
    assert!(target.dispatch(event, &()).unwrap().is_transitioned());
}

#[test]
fn cross_family_mapping_rejects_incompatible_payload_signatures() {
    let definition = source_definition();
    let source = definition.instantiate(Instance(1), ()).unwrap();
    let target_definition = receiver::<u16>();
    let target = target_definition.instantiate(2, ()).unwrap();
    let source_address = InstanceAddress::<OrderIds>::new(5, Instance(1));
    let target_address = InstanceAddress::<ReceiverIds>::new(6, 2);
    let sources = InstanceRegistry::builder()
        .instance(source_address.clone(), &source)
        .unwrap()
        .build();
    let targets = InstanceRegistry::builder()
        .instance(target_address.clone(), &target)
        .unwrap()
        .build();
    let result = Composition::<u8, OrderIds, ReceiverIds>::builder(sources, targets)
        .route(
            Route::builder(source_address, Input::Finish)
                .to(target_address)
                .as_event(50)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build();
    assert!(matches!(
        result,
        Err(CompositionBuildError::DestinationPayloadMismatch)
    ));
}

struct SameLabels;
impl DiagramLabels<ReceiverIds> for SameLabels {
    fn state<'a>(&'a self, _: &'a bool) -> &'a str {
        "same"
    }
    fn transition<'a>(&'a self, _: &'a u16) -> &'a str {
        "same"
    }
    fn event<'a>(&'a self, _: &'a u16) -> &'a str {
        "same"
    }
    fn condition<'a>(&'a self, _: &'a u16) -> &'a str {
        "same"
    }
    fn disposition<'a>(&'a self, _: &'a u16) -> &'a str {
        "same"
    }
}

#[test]
fn identical_diagram_labels_do_not_merge_distinct_identities() {
    let definition = receiver::<()>();
    let diagram = definition
        .diagram_with_labels(&InspectionLimits::builder().build().unwrap(), &SameLabels)
        .unwrap();
    assert!(diagram.contains("state \"same\" as s0"));
    assert!(diagram.contains("state \"same\" as s1"));
    assert!(diagram.contains("s0 --> s1 : same / same"));
}
