use crate::{
    DispatchError,
    DispatchOutcome,
    Event,
    MachineIds,
    MachineInstance,
};

pub(super) trait Target<E, F, X, I: MachineIds> {
    fn step(
        &mut self,
        event: &Event<E, I>,
        facts: &F,
        available: usize,
        max_outputs: usize,
    ) -> Result<Option<DispatchOutcome<X, E, I>>, DispatchError<I>>;
}
impl<D: crate::FallibleCandidateData, E, F, X, I: MachineIds> Target<E, F, X, I>
    for MachineInstance<'_, D, E, F, X, E, I>
{
    fn step(
        &mut self,
        event: &Event<E, I>,
        facts: &F,
        available: usize,
        max_outputs: usize,
    ) -> Result<Option<DispatchOutcome<X, E, I>>, DispatchError<I>> {
        let decision = self.evaluate_borrowed_at(self.revision(), event, facts)?;
        let (effects, outputs) = decision.intent_counts();
        if effects + outputs > available || outputs > max_outputs {
            return Ok(None);
        }
        self.accept(decision).map(Some)
    }
}
