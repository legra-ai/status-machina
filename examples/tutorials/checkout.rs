//! Explicit synchronous routing between independently owned machines.
use status_machina::{
    CapabilitySnapshot,
    Composition,
    Delivery,
    Effect,
    Event,
    InstanceAddress,
    InstanceRegistry,
    LocalAction,
    MachineDefinition,
    PropagationLimits,
    PropagationStatus,
    Route,
    StateDefinition,
    Transition,
};

use super::model::*;
use super::order;

pub(crate) type Invoice = MachineDefinition<u64, Request, Facts, Work, Request, Ids>;

pub(crate) fn definition() -> Invoice {
    Invoice::builder_with_effects_and_output_events(Definition::Invoice)
        .initial(State::Idle)
        .state(StateDefinition::builder(State::Idle).build())
        .unwrap()
        .state(StateDefinition::builder(State::Issued).terminal().build())
        .unwrap()
        .transition(
            Transition::builder_with_effects_and_output_events(Edge::IssueInvoice)
                .from(State::Idle)
                .on(Input::InvoiceRequested)
                .to(State::Issued)
                .action(
                    LocalAction::builder(1)
                        .apply(|data: &mut u64, request: &Request, _: &Facts| {
                            *data = request.order;
                            Ok(())
                        })
                        .build()
                        .unwrap(),
                )
                .unwrap()
                .effect(
                    Effect::builder(3)
                        .describe(|order: &u64, _: &Request, _: &Facts| Work::SendInvoice {
                            order: *order,
                        })
                        .build()
                        .unwrap(),
                )
                .unwrap()
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}

pub(crate) struct Summary {
    pub(crate) accepted_steps: usize,
    pub(crate) order_state: State,
    pub(crate) invoice_state: State,
    pub(crate) work: Vec<Work>,
}

pub(crate) fn run() -> Summary {
    let order_definition = order::definition();
    let invoice_definition = definition();
    let mut order = order_definition
        .instantiate(Instance(7), OrderData::empty())
        .unwrap();
    let mut invoice = invoice_definition.instantiate(Instance(8), 0).unwrap();
    let order_address = InstanceAddress::<Ids>::new(1, Instance(7));
    let invoice_address = InstanceAddress::<Ids>::new(1, Instance(8));
    let registry = InstanceRegistry::builder()
        .instance(order_address.clone(), &order)
        .unwrap()
        .instance(invoice_address.clone(), &invoice)
        .unwrap()
        .build();
    let composition = Composition::builder(registry.clone(), registry)
        .route(
            Route::builder(order_address.clone(), Input::InvoiceRequested)
                .to(invoice_address.clone())
                .as_event(Input::InvoiceRequested)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let limits = PropagationLimits::builder()
        .max_steps(2)
        .max_intents(4)
        .build()
        .unwrap();
    let mut bound = composition
        .bind()
        .instance(order_address.clone(), &mut order)
        .unwrap()
        .instance(invoice_address, &mut invoice)
        .unwrap()
        .build()
        .unwrap();
    let report = bound.propagate(
        vec![Delivery::new(
            order_address,
            Event::new(
                Input::Submit,
                Request {
                    order: 7,
                    quantity: 2,
                },
            ),
        )],
        &Facts {
            stock_available: true,
        },
        &CapabilitySnapshot::builder().build(),
        limits,
    );
    assert_eq!(report.status, PropagationStatus::Quiescent);
    assert!(report.pending.is_empty());
    assert!(report.unrouted.is_empty());
    // Release the exclusive bindings before the caller can execute external
    // work.
    drop(bound);
    Summary {
        accepted_steps: report.steps.len(),
        order_state: *order.state(),
        invoice_state: *invoice.state(),
        // bounded: the propagation budget caps retained intents at four.
        work: report
            .effects
            .into_iter()
            .map(|effect| effect.intent.into_parts().1)
            .collect(),
    }
}
