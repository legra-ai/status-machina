use std::collections::VecDeque;

use super::super::{
    CapabilitySnapshot,
    InstanceAddress,
    RoutedItem,
};
use super::{
    AddressedEffect,
    BoundComposition,
    Delivery,
    PropagationLimits,
    PropagationReport,
    PropagationStatus,
    PropagationStep,
};
use crate::{
    DispatchOutcome,
    MachineIds,
};

impl<E, F, X, I: MachineIds> BoundComposition<'_, E, F, X, I> {
    /// Dispatch and propagate in the original caller's stack, without
    /// recursion.
    ///
    /// Facts and advisory capabilities are fixed for this call. Return first,
    /// release instance borrows/caller locks, then execute external effects.
    /// A later call may submit pending inputs or correlated completions with
    /// fresh observations. No callbacks execute merely by retaining a report.
    ///
    /// Capacity failure preserves the proposed input without accepting its
    /// transition. Earlier accepted transitions and their effects remain valid.
    ///
    /// # Panics
    /// Propagates caller callback panics; external effects are never rolled
    /// back. Also panics if the internal FIFO loses its inspected front entry.
    pub fn propagate(
        &mut self,
        inputs: Vec<Delivery<E, I>>,
        facts: &F,
        capabilities: &CapabilitySnapshot<I>,
        limits: PropagationLimits,
    ) -> PropagationReport<E, X, I> {
        let mut report = PropagationReport::new();
        if inputs.len() > limits.intents {
            report.status = PropagationStatus::IntentLimit;
            report.pending = inputs;
            return report;
        }
        // bounded: inputs checked above; each acceptance reserves new intents.
        let mut pending: VecDeque<_> = inputs.into();
        while let Some(delivery) = pending.front() {
            if report.steps.len() == limits.steps {
                report.status = PropagationStatus::StepLimit;
                break;
            }
            let Some(target) = self.targets.get_mut(&delivery.destination) else {
                report.status = PropagationStatus::UnboundDestination;
                break;
            };
            let available =
                limits.intents - (pending.len() - 1 + report.effects.len() + report.unrouted.len());
            let outcome = match target.step(
                &delivery.event,
                facts,
                available,
                self.composition.max_batch,
            ) {
                Ok(Some(outcome)) => outcome,
                Ok(None) => {
                    report.status = PropagationStatus::IntentLimit;
                    break;
                }
                Err(error) => {
                    report.status = PropagationStatus::DispatchFailed(error);
                    break;
                }
            };
            let delivery = pending.pop_front().expect("front still present");
            let metadata = self.record_outcome(
                &delivery.destination,
                outcome,
                capabilities,
                &mut pending,
                &mut report,
            );
            report.steps.push(PropagationStep {
                destination: delivery.destination,
                outcome: metadata,
            });
        }
        // bounded: accepted intentions plus pending inputs never exceed budget.
        report.pending = pending.into();
        report
    }

    fn record_outcome(
        &self,
        source: &InstanceAddress<I>,
        outcome: DispatchOutcome<X, E, I>,
        capabilities: &CapabilitySnapshot<I>,
        pending: &mut VecDeque<Delivery<E, I>>,
        report: &mut PropagationReport<E, X, I>,
    ) -> DispatchOutcome<(), (), I> {
        match outcome {
            DispatchOutcome::Transitioned {
                record,
                effects,
                outputs,
            } => {
                report
                    .effects
                    .extend(effects.into_iter().map(|intent| AddressedEffect {
                        source: source.clone(),
                        intent,
                    }));
                for output in outputs {
                    match self.composition.route_one(source, output, capabilities) {
                        RoutedItem::Routed {
                            destination, event, ..
                        } => pending.push_back(Delivery::new(destination, event)),
                        unrouted => report.unrouted.push(unrouted),
                    }
                }
                DispatchOutcome::Transitioned {
                    record,
                    effects: Vec::new(),
                    outputs: Vec::new(),
                }
            }
            DispatchOutcome::Refused { state, event } => DispatchOutcome::Refused { state, event },
            DispatchOutcome::Ignored {
                disposition,
                state,
                event,
                reason,
            } => DispatchOutcome::Ignored {
                disposition,
                state,
                event,
                reason,
            },
        }
    }
}
