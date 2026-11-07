use std::sync::Arc;

use crate::MachineDefinition;

/// Borrowed or shared ownership of an immutable machine definition.
pub(crate) enum MachineInstanceDefinition<
    'definition,
    D,
    E,
    F,
    X,
    O,
    I: crate::MachineIds = crate::DefaultIds,
> {
    Borrowed(&'definition MachineDefinition<D, E, F, X, O, I>),
    Shared(Arc<MachineDefinition<D, E, F, X, O, I>>),
}

impl<D, E, F, X, O, I: crate::MachineIds> MachineInstanceDefinition<'_, D, E, F, X, O, I> {
    pub(super) fn definition(&self) -> &MachineDefinition<D, E, F, X, O, I> {
        match self {
            Self::Borrowed(definition) => definition,
            Self::Shared(definition) => definition,
        }
    }
}
