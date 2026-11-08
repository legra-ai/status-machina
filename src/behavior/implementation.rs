use std::sync::Arc;

use super::BehaviorRole;
use super::invariant::InvariantPredicate;
use crate::{
    CapabilityStatus,
    ScenarioObservation,
};

type CapabilityCallback<D, F, I> = Arc<
    dyn Fn(
            &<I as crate::MachineIds>::State,
            &D,
            &F,
        ) -> Result<CapabilityStatus, crate::EvaluationError>
        + Send
        + Sync,
>;
type AssertionCallback<I> =
    Arc<dyn Fn(&ScenarioObservation<I>) -> Result<bool, crate::EvaluationError> + Send + Sync>;

pub(super) enum Implementation<D, E, F, X, O, I: crate::MachineIds> {
    Action(crate::action::ActionOperation<D, E, F>),
    Condition(crate::condition::Predicate<D, E, F>),
    Invariant(InvariantPredicate<D>),
    Effect(crate::transition::EffectConstructor<D, E, F, X>),
    OutputEvent(crate::output_event::OutputEventConstructor<D, E, F, O>),
    Capability(CapabilityCallback<D, F, I>),
    Assertion(AssertionCallback<I>),
}
impl<D, E, F, X, O, I: crate::MachineIds> Implementation<D, E, F, X, O, I> {
    pub(super) const fn role(&self) -> BehaviorRole {
        match self {
            Self::Action(_) => BehaviorRole::Action,
            Self::Condition(_) => BehaviorRole::Condition,
            Self::Invariant(_) => BehaviorRole::Invariant,
            Self::Effect(_) => BehaviorRole::Effect,
            Self::OutputEvent(_) => BehaviorRole::OutputEvent,
            Self::Capability(_) => BehaviorRole::Capability,
            Self::Assertion(_) => BehaviorRole::Assertion,
        }
    }
}
