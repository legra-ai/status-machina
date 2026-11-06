use super::{
    InspectionError,
    InspectionLimits,
    MachineDefinition,
};

/// Structural graph inventory, not a claim that any edge was executed.
#[derive(Debug, Eq, PartialEq)]
pub struct StructuralCoverage<I: crate::MachineIds = crate::DefaultIds> {
    states: Vec<StateCoverage<I>>,
    transitions: Vec<I::Transition>,
    dispositions: Vec<I::Disposition>,
}

impl<I: crate::MachineIds> StructuralCoverage<I> {
    /// Return states in identifier order.
    #[must_use]
    pub fn states(&self) -> &[StateCoverage<I>] {
        &self.states
    }
    /// Return declared transitions in identifier order.
    #[must_use]
    pub fn transitions(&self) -> &[I::Transition] {
        &self.transitions
    }
    /// Return declared dispositions in identifier order.
    #[must_use]
    pub fn dispositions(&self) -> &[I::Disposition] {
        &self.dispositions
    }
}

/// Incoming/outgoing structural counts for one declared state.
#[derive(Debug, Eq, PartialEq)]
pub struct StateCoverage<I: crate::MachineIds = crate::DefaultIds> {
    id: I::State,
    initial: bool,
    terminal: bool,
    incoming: usize,
    outgoing: usize,
    dispositions: usize,
}

impl<I: crate::MachineIds> StateCoverage<I> {
    /// Return the state identifier.
    #[must_use]
    pub const fn id(&self) -> &I::State {
        &self.id
    }
    /// Whether this is the initial state.
    #[must_use]
    pub const fn is_initial(&self) -> bool {
        self.initial
    }
    /// Whether this is declared terminal.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }
    /// Count incoming transitions, including self-transitions.
    #[must_use]
    pub const fn incoming(&self) -> usize {
        self.incoming
    }
    /// Count outgoing transitions, including self-transitions.
    #[must_use]
    pub const fn outgoing(&self) -> usize {
        self.outgoing
    }
    /// Count declared harmless-event dispositions at this state.
    #[must_use]
    pub const fn dispositions(&self) -> usize {
        self.dispositions
    }
}

impl<D, E, F, X, O, I: crate::MachineIds> MachineDefinition<D, E, F, X, O, I>
where
    I::State: Ord,
    I::Transition: Ord,
    I::Disposition: Ord,
{
    /// Inspect graph structure without running predicates or actions.
    ///
    /// This inventory cannot establish guard satisfiability or execution
    /// coverage.
    ///
    /// # Errors
    /// Rejects a graph whose total records exceed the inspection entry budget.
    pub fn structural_coverage(
        &self,
        limits: &InspectionLimits,
    ) -> Result<StructuralCoverage<I>, InspectionError> {
        self.check_inspection_limit(limits)?;
        // bounded: the combined output records passed the caller's entry limit.
        let mut states: Vec<StateCoverage<I>> = self
            .states
            .iter()
            .map(|state| StateCoverage {
                id: state.id.clone(),
                initial: state.id == self.initial,
                terminal: state.terminal(),
                incoming: self
                    .transitions
                    .iter()
                    .filter(|edge| edge.to == state.id)
                    .count(),
                outgoing: self
                    .transitions
                    .iter()
                    .filter(|edge| edge.from == state.id)
                    .count(),
                dispositions: self
                    .dispositions
                    .iter()
                    .filter(|rule| rule.from == state.id)
                    .count(),
            })
            .collect();
        states.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        // bounded: combined records checked before allocation.
        let mut transitions: Vec<_> = self
            .transitions
            .iter()
            .map(|edge| edge.id.clone())
            .collect();
        transitions.sort_unstable();
        // bounded: combined records checked before allocation.
        let mut dispositions: Vec<_> = self
            .dispositions
            .iter()
            .map(|rule| rule.id.clone())
            .collect();
        dispositions.sort_unstable();
        Ok(StructuralCoverage {
            states,
            transitions,
            dispositions,
        })
    }
}
