//! Public API tests for caller-owned output-event intents.

use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use status_machina::{
    Condition,
    ConditionId,
    Event,
    EventId,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    OutputEvent,
    OutputEventBuildError,
    StateDefinition,
    StateId,
    Transition,
    TransitionBuildError,
    TransitionId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Request {
    correlation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Response {
    correlation: u64,
    message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Facts {
    allowed: bool,
    accept_postcondition: bool,
}

fn definition(
    constructor_calls: Arc<AtomicUsize>,
) -> MachineDefinition<(), Request, Facts, (), Response> {
    let waiting = StateId::try_new("waiting").unwrap();
    let complete = StateId::try_new("complete").unwrap();
    let request_event = EventId::try_new("request").unwrap();
    let response_event = EventId::try_new("response").unwrap();
    let output = OutputEvent::builder(response_event)
        .describe(move |_data: &(), request: &Request, _facts: &Facts| {
            constructor_calls.fetch_add(1, Ordering::SeqCst);
            Response {
                correlation: request.correlation,
                message: "accepted".to_owned(),
            }
        })
        .build()
        .unwrap();
    let postcondition = Condition::builder(ConditionId::try_new("accept-response").unwrap())
        .evaluate(|_data: &(), _request: &Request, facts: &Facts| facts.accept_postcondition)
        .build()
        .unwrap();
    let transition = Transition::<(), Request, Facts, (), Response>::builder_with_output_events(
        TransitionId::try_new("complete-request").unwrap(),
    )
    .from(waiting.clone())
    .on(request_event)
    .to(complete.clone())
    .when(
        Condition::builder(ConditionId::try_new("allow-request").unwrap())
            .evaluate(|_data: &(), _request: &Request, facts: &Facts| facts.allowed)
            .build()
            .unwrap(),
    )
    .postcondition(postcondition)
    .unwrap()
    .output_event(output)
    .unwrap()
    .build()
    .unwrap();

    MachineDefinition::<(), Request, Facts, (), Response>::builder_with_output_events(
        MachineId::try_new("request-flow").unwrap(),
    )
    .initial(waiting.clone())
    .state(StateDefinition::builder(waiting).build())
    .unwrap()
    .state(StateDefinition::builder(complete).build())
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap()
}

#[test]
fn output_event_is_returned_only_after_transition_acceptance() {
    let constructor_calls = Arc::new(AtomicUsize::new(0));
    let definition = definition(Arc::clone(&constructor_calls));
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::from_u128(
                0x0000_0000_0000_7000_8000_0000_0000_0001,
            ))
            .unwrap(),
            (),
        )
        .unwrap();
    let request_event = EventId::try_new("request").unwrap();

    let refused = instance
        .dispatch(
            Event::new(request_event.clone(), Request { correlation: 7 }),
            &Facts {
                allowed: false,
                accept_postcondition: true,
            },
        )
        .unwrap();
    assert!(refused.is_refused());
    assert_eq!(constructor_calls.load(Ordering::SeqCst), 0);
    assert_eq!(instance.revision(), 0);

    let rejected_candidate = instance.dispatch(
        Event::new(request_event.clone(), Request { correlation: 8 }),
        &Facts {
            allowed: true,
            accept_postcondition: false,
        },
    );
    assert!(rejected_candidate.is_err());
    assert_eq!(constructor_calls.load(Ordering::SeqCst), 0);
    assert_eq!(instance.state().as_str(), "waiting");
    assert_eq!(instance.revision(), 0);

    let accepted = instance
        .dispatch(
            Event::new(request_event, Request { correlation: 9 }),
            &Facts {
                allowed: true,
                accept_postcondition: true,
            },
        )
        .unwrap();

    let status_machina::DispatchOutcome::Transitioned { outputs, .. } = accepted else {
        panic!("accepted transition did not produce a transition outcome");
    };
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].id().as_str(), "response");
    assert_eq!(
        outputs[0].payload(),
        &Response {
            correlation: 9,
            message: "accepted".to_owned(),
        }
    );
    assert_eq!(constructor_calls.load(Ordering::SeqCst), 1);
    assert_eq!(instance.state().as_str(), "complete");
    assert_eq!(instance.revision(), 1);
}

#[test]
fn output_event_builder_requires_a_constructor_and_enforces_its_bound() {
    assert_eq!(
        OutputEvent::<(), Request, Facts, Response>::builder(
            EventId::try_new("response").unwrap(),
        )
        .build()
        .err(),
        Some(OutputEventBuildError::MissingConstructor)
    );

    let source = StateId::try_new("waiting").unwrap();
    let destination = StateId::try_new("complete").unwrap();
    let mut transition =
        Transition::<(), Request, Facts, (), Response>::builder_with_output_events(
            TransitionId::try_new("emit-many").unwrap(),
        )
        .from(source)
        .on(EventId::try_new("request").unwrap())
        .to(destination);

    for index in 0..16 {
        let output = OutputEvent::builder(EventId::try_new(format!("response-{index}")).unwrap())
            .describe(|_data: &(), request: &Request, _facts: &Facts| Response {
                correlation: request.correlation,
                message: "accepted".to_owned(),
            })
            .build()
            .unwrap();
        transition = transition.output_event(output).unwrap();
    }

    let overflow = OutputEvent::builder(EventId::try_new("overflow").unwrap())
        .describe(|_data: &(), request: &Request, _facts: &Facts| Response {
            correlation: request.correlation,
            message: "accepted".to_owned(),
        })
        .build()
        .unwrap();
    assert!(matches!(
        transition.output_event(overflow),
        Err(TransitionBuildError::OutputEventLimitExceeded)
    ));
}
