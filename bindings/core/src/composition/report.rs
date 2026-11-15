//! Serializable routing items and propagation reports, addressed by table
//! index. Payloads stay leased in their language arenas.
use serde::Serialize;
use status_machina::{
    AddressedEffect,
    Delivery,
    PropagationReport,
    PropagationStatus,
    PropagationStep,
    RoutedItem,
};

use super::propagation::Propagated;
use super::topology::Topology;
use crate::error::Error;
use crate::ids::Ids;
use crate::runtime::{
    Outcome,
    status_name,
};
use crate::value::{
    Host,
    Value,
    ValueKey,
};

/// One routing result. `source` and `destination` are registered table
/// indexes, absent for an address outside both registries.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    Routed {
        source: Option<u32>,
        destination: Option<u32>,
        event: u32,
        payload: ValueKey,
    },
    Unmatched {
        source: Option<u32>,
        event: u32,
        payload: ValueKey,
    },
    Deferred {
        source: Option<u32>,
        destination: Option<u32>,
        capability: u32,
        status: &'static str,
        event: u32,
        payload: ValueKey,
    },
}
impl Item {
    pub(super) fn new<H: Host>(item: RoutedItem<Value<H>, Ids<H>>, topology: &Topology<H>) -> Self {
        match item {
            RoutedItem::Routed {
                source,
                destination,
                event,
            } => Self::Routed {
                source: topology.index(&source),
                destination: topology.index(&destination),
                event: *event.id(),
                payload: event.payload().key(),
            },
            RoutedItem::Unmatched { source, intent } => Self::Unmatched {
                source: topology.index(&source),
                event: *intent.id(),
                payload: intent.payload().key(),
            },
            RoutedItem::Deferred {
                source,
                destination,
                capability,
                status,
                intent,
            } => Self::Deferred {
                source: topology.index(&source),
                destination: topology.index(&destination),
                capability,
                status: status_name(status),
                event: *intent.id(),
                payload: intent.payload().key(),
            },
        }
    }
}
/// A completed dispatch; `input` is the caller input position it consumed.
#[derive(Serialize)]
pub struct Step {
    pub input: Option<usize>,
    pub destination: Option<u32>,
    pub outcome: Outcome,
}
#[derive(Debug, Serialize)]
pub struct Effect {
    pub source: Option<u32>,
    pub id: u32,
    pub payload: ValueKey,
}
/// Remaining work; `input` is the caller input position when it was supplied.
#[derive(Debug, Serialize)]
pub struct Pending {
    pub input: Option<usize>,
    pub destination: Option<u32>,
    pub event: u32,
    pub payload: ValueKey,
}
#[derive(Serialize)]
pub struct Report {
    pub status: &'static str,
    pub steps: Vec<Step>,
    pub effects: Vec<Effect>,
    pub unrouted: Vec<Item>,
    pub pending: Vec<Pending>,
}
impl<H: Host> Topology<H> {
    /// Address the kernel report. FIFO order makes the first
    /// `min(steps, inputs)` steps consume the caller inputs in order, and the
    /// unconsumed caller inputs lead `pending`.
    pub(super) fn report(
        &self,
        report: PropagationReport<Value<H>, Value<H>, Ids<H>>,
        inputs: usize,
    ) -> Propagated<H> {
        let PropagationReport {
            status,
            steps,
            effects,
            unrouted,
            pending,
        } = report;
        let consumed = steps.len().min(inputs);
        let (status, failure) = match status {
            PropagationStatus::Quiescent => ("quiescent", None),
            PropagationStatus::StepLimit => ("step_limit", None),
            PropagationStatus::IntentLimit => ("intent_limit", None),
            PropagationStatus::UnboundDestination => ("unbound_destination", None),
            PropagationStatus::DispatchFailed(error) => {
                ("dispatch_failed", Some(Error::Dispatch(error)))
            }
        };
        // bounded: every vector below is capped by the propagation limits.
        let report = Report {
            status,
            steps: steps
                .into_iter()
                .enumerate()
                .map(|(position, step)| self.step(position, consumed, step))
                .collect(),
            effects: effects
                .into_iter()
                .map(|effect| self.effect(effect))
                .collect(),
            unrouted: unrouted
                .into_iter()
                .map(|item| Item::new(item, self))
                .collect(),
            pending: pending
                .into_iter()
                .enumerate()
                .map(|(position, delivery)| {
                    let input = (position < inputs - consumed).then_some(consumed + position);
                    self.pending(input, delivery)
                })
                .collect(),
        };
        Propagated { report, failure }
    }
    fn step(&self, position: usize, consumed: usize, step: PropagationStep<Ids<H>>) -> Step {
        Step {
            input: (position < consumed).then_some(position),
            destination: self.index(&step.destination),
            outcome: step.outcome.into(),
        }
    }
    fn effect(&self, effect: AddressedEffect<Value<H>, Ids<H>>) -> Effect {
        let (id, payload) = effect.intent.into_parts();
        Effect {
            source: self.index(&effect.source),
            id,
            payload: payload.key(),
        }
    }
    fn pending(&self, input: Option<usize>, delivery: Delivery<Value<H>, Ids<H>>) -> Pending {
        let (destination, event) = delivery.into_parts();
        Pending {
            input,
            destination: self.index(&destination),
            event: *event.id(),
            payload: event.payload().key(),
        }
    }
}
