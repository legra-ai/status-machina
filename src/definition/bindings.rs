use super::MachineDefinition;

impl<D, E, F, X, O, I: crate::MachineIds> MachineDefinition<D, E, F, X, O, I> {
    /// Inspect every registered Story/implementation reference before
    /// instantiation.
    ///
    /// Iteration includes repeated use sites and nested condition leaves, in
    /// declaration order. Directly supplied inline callbacks have no registered
    /// reference and are omitted; this iterator does not certify full
    /// portability. It neither allocates a whole-definition manifest nor
    /// executes callbacks.
    pub fn bindings(&self) -> impl Iterator<Item = &crate::BehaviorRef<I>> {
        self.states
            .iter()
            .flat_map(super::StateDefinition::bindings)
            .chain(
                self.transitions
                    .iter()
                    .flat_map(crate::Transition::bindings),
            )
            .chain(
                self.dispositions
                    .iter()
                    .flat_map(|rule| rule.condition.binding_refs()),
            )
    }
}
