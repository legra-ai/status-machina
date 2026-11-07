use super::{
    DispatchError,
    DispatchOutcome,
    EffectIntent,
    MachineInstance,
    TransitionRecord,
};
use crate::{
    EvaluationPhase,
    Event,
    MachineDecision,
    OutputEventIntent,
    TransitionKind,
};

impl<D: crate::FallibleCandidateData, E, F, X, O, I: crate::MachineIds>
    MachineInstance<'_, D, E, F, X, O, I>
{
    /// Evaluate an event without changing the accepted instance.
    ///
    /// The owned decision retains candidate data and unexecuted intents until
    /// checked acceptance. Dropping it leaves this instance unchanged.
    ///
    /// # Errors
    /// Returns selection, action, postcondition, invariant or revision errors.
    pub fn evaluate(
        &self,
        event: Event<E, I>,
        facts: &F,
    ) -> Result<MachineDecision<D, X, O, I>, DispatchError<I>> {
        self.evaluate_at(self.revision, event, facts)
    }

    /// Evaluate only at the caller's observed revision, before invoking
    /// callbacks.
    ///
    /// # Errors
    /// Rejects a stale revision or any failed evaluation check.
    pub fn evaluate_at(
        &self,
        expected_revision: u64,
        event: Event<E, I>,
        facts: &F,
    ) -> Result<MachineDecision<D, X, O, I>, DispatchError<I>> {
        self.evaluate_borrowed_at(expected_revision, &event, facts)
    }

    pub(crate) fn evaluate_borrowed_at(
        &self,
        expected_revision: u64,
        event: &Event<E, I>,
        facts: &F,
    ) -> Result<MachineDecision<D, X, O, I>, DispatchError<I>> {
        if expected_revision != self.revision {
            return Err(DispatchError::StaleRevision {
                expected: expected_revision,
                current: self.revision,
            });
        }
        let mut selected_transition = None;
        for transition in self.definition().transitions() {
            if transition.from != self.state || transition.event != *event.id() {
                continue;
            }
            let enabled = match &transition.guard {
                Some(guard) => {
                    guard
                        .evaluate(&self.data, event.payload(), facts)
                        .map_err(|error| DispatchError::EvaluationFailed {
                            phase: EvaluationPhase::Guard,
                            error,
                        })?
                }
                None => true,
            };
            if !enabled {
                continue;
            }
            if selected_transition.replace(transition).is_some() {
                return Err(DispatchError::Ambiguous {
                    state: self.state.clone(),
                });
            }
        }

        let mut selected_disposition = None;
        for disposition in self.definition().dispositions() {
            if disposition.from != self.state
                || disposition.event != *event.id()
                || !disposition
                    .condition
                    .evaluate(&self.data, event.payload(), facts)
                    .map_err(|error| DispatchError::EvaluationFailed {
                        phase: EvaluationPhase::Disposition,
                        error,
                    })?
            {
                continue;
            }
            if selected_transition.is_some() || selected_disposition.replace(disposition).is_some()
            {
                return Err(DispatchError::Ambiguous {
                    state: self.state.clone(),
                });
            }
        }

        let Some(transition) = selected_transition else {
            if let Some(disposition) = selected_disposition {
                return Ok(self.decision(
                    None,
                    DispatchOutcome::Ignored {
                        disposition: disposition.id.clone(),
                        state: self.state.clone(),
                        event: event.id().clone(),
                        reason: disposition.reason.clone(),
                    },
                ));
            }
            return Ok(self.decision(
                None,
                DispatchOutcome::Refused {
                    state: self.state.clone(),
                    event: event.id().clone(),
                },
            ));
        };

        let mut candidate =
            self.data
                .try_snapshot()
                .map_err(|error| DispatchError::EvaluationFailed {
                    phase: EvaluationPhase::Snapshot,
                    error,
                })?;
        self.apply_transition_actions(transition, &mut candidate, event.payload(), facts)?;
        for condition in &transition.postconditions {
            if !condition
                .evaluate(&candidate, event.payload(), facts)
                .map_err(|error| DispatchError::EvaluationFailed {
                    phase: EvaluationPhase::Postcondition,
                    error,
                })?
            {
                return Err(DispatchError::PostconditionFailed {
                    condition: condition.id().clone(),
                });
            }
        }
        if let Some(state) = self.definition().state(&transition.to)
            && !state
                .accepts(&candidate)
                .map_err(|error| DispatchError::EvaluationFailed {
                    phase: EvaluationPhase::Invariant,
                    error,
                })?
        {
            return Err(DispatchError::InvariantFailed {
                state: transition.to.clone(),
            });
        }
        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or(DispatchError::RevisionExhausted)?;
        let effects = self.construct_effects(transition, &candidate, event.payload(), facts)?;
        // bounded: transition construction caps output events at 16.
        let outputs = transition
            .output_events
            .iter()
            .map(|output_event| {
                Ok(OutputEventIntent::new(
                    output_event.id.clone(),
                    (output_event.construct)(&candidate, event.payload(), facts).map_err(
                        |error| DispatchError::EvaluationFailed {
                            phase: EvaluationPhase::Output,
                            error,
                        },
                    )?,
                ))
            })
            .collect::<Result<Vec<_>, DispatchError<I>>>()?;
        let candidate =
            candidate
                .prepare_accept()
                .map_err(|error| DispatchError::EvaluationFailed {
                    phase: EvaluationPhase::Snapshot,
                    error,
                })?;
        let record = TransitionRecord {
            transition: transition.id.clone(),
            from: self.state.clone(),
            to: transition.to.clone(),
            previous_revision: self.revision,
            revision: next_revision,
        };

        Ok(self.decision(
            Some(candidate),
            DispatchOutcome::Transitioned {
                record,
                effects,
                outputs,
            },
        ))
    }

    fn decision(
        &self,
        candidate: Option<D>,
        outcome: DispatchOutcome<X, O, I>,
    ) -> MachineDecision<D, X, O, I> {
        MachineDecision::new(
            self.definition().id().clone(),
            self.instance_id.clone(),
            self.revision,
            candidate,
            outcome,
        )
    }

    fn construct_effects(
        &self,
        transition: &crate::Transition<D, E, F, X, O, I>,
        candidate: &D,
        payload: &E,
        facts: &F,
    ) -> Result<Vec<EffectIntent<X, I>>, DispatchError<I>> {
        let (exits, entries) = if transition.kind == TransitionKind::External {
            let source = self
                .definition()
                .state(&transition.from)
                .expect("validated source exists");
            let destination = self
                .definition()
                .state(&transition.to)
                .expect("validated destination exists");
            (source.exit_effects(), destination.entry_effects())
        } else {
            (&[][..], &[][..])
        };
        // bounded: at most 16 exits + 16 transition effects + 16 entries.
        exits
            .iter()
            .chain(&transition.effects)
            .chain(entries)
            .map(|effect| {
                Ok(EffectIntent {
                    id: effect.id.clone(),
                    payload: (effect.construct)(candidate, payload, facts).map_err(|error| {
                        DispatchError::EvaluationFailed {
                            phase: EvaluationPhase::Effect,
                            error,
                        }
                    })?,
                })
            })
            .collect()
    }

    fn apply_transition_actions(
        &self,
        transition: &crate::Transition<D, E, F, X, O, I>,
        candidate: &mut D,
        payload: &E,
        facts: &F,
    ) -> Result<(), DispatchError<I>> {
        if transition.kind == TransitionKind::External {
            let source = self
                .definition()
                .state(&transition.from)
                .expect("validated transition source exists");
            self.apply_actions(source.exit_actions(), candidate, payload, facts)?;
        }
        self.apply_actions(&transition.actions, candidate, payload, facts)?;
        if transition.kind == TransitionKind::External {
            let destination = self
                .definition()
                .state(&transition.to)
                .expect("validated transition destination exists");
            self.apply_actions(destination.entry_actions(), candidate, payload, facts)?;
        }
        Ok(())
    }

    fn apply_actions(
        &self,
        actions: &[crate::LocalAction<D, E, F, I>],
        candidate: &mut D,
        payload: &E,
        facts: &F,
    ) -> Result<(), DispatchError<I>> {
        for action in actions {
            (action.apply)(candidate, payload, facts)
                .map_err(|message| DispatchError::ActionFailed { message })?;
        }
        Ok(())
    }
}
