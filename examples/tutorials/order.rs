//! A guarded order accepts pure candidate changes and returns external work.
use status_machina::{
    Condition,
    Effect,
    LocalAction,
    MachineDefinition,
    OutputEvent,
    StateDefinition,
    Transition,
};

use super::model::*;

pub(crate) type Order = MachineDefinition<OrderData, Request, Facts, Work, Request, Ids>;

pub(crate) fn definition() -> Order {
    Order::builder_with_effects_and_output_events(Definition::Order)
        .initial(State::Draft)
        .state(StateDefinition::builder(State::Draft).build())
        .unwrap()
        .state(
            StateDefinition::builder(State::Accepted)
                .invariant(|data: &OrderData| data.quantity > 0)
                .build(),
        )
        .unwrap()
        .state(StateDefinition::builder(State::Shipped).terminal().build())
        .unwrap()
        .state(
            StateDefinition::builder(State::Cancelled)
                .terminal()
                .build(),
        )
        .unwrap()
        .transition(
            Transition::builder_with_effects_and_output_events(Edge::Submit)
                .from(State::Draft)
                .on(Input::Submit)
                .to(State::Accepted)
                .when(
                    Condition::builder(Guard::StockAvailable)
                        .evaluate(|_: &OrderData, _: &Request, facts: &Facts| facts.stock_available)
                        .build()
                        .unwrap(),
                )
                .action(
                    LocalAction::builder(1)
                        .apply(|data: &mut OrderData, request: &Request, _: &Facts| {
                            data.order = request.order;
                            data.quantity = request.quantity;
                            Ok(())
                        })
                        .build()
                        .unwrap(),
                )
                .unwrap()
                .effect(
                    Effect::builder(1)
                        .describe(|data: &OrderData, _: &Request, _: &Facts| {
                            Work::SendConfirmation {
                                order: data.order,
                                quantity: data.quantity,
                            }
                        })
                        .build()
                        .unwrap(),
                )
                .unwrap()
                .output_event(
                    OutputEvent::builder(Input::InvoiceRequested)
                        .describe(|_: &OrderData, request: &Request, _: &Facts| *request)
                        .build()
                        .unwrap(),
                )
                .unwrap()
                .build()
                .unwrap(),
        )
        .unwrap()
        .transition(
            Transition::builder_with_effects_and_output_events(Edge::Ship)
                .from(State::Accepted)
                .on(Input::Ship)
                .to(State::Shipped)
                .effect(
                    Effect::builder(2)
                        .describe(
                            |data: &OrderData, _: &Request, _: &Facts| Work::DispatchParcel {
                                order: data.order,
                            },
                        )
                        .build()
                        .unwrap(),
                )
                .unwrap()
                .build()
                .unwrap(),
        )
        .unwrap()
        .transition(
            Transition::builder_with_effects_and_output_events(Edge::Cancel)
                .from(State::Draft)
                .on(Input::Cancel)
                .to(State::Cancelled)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}
