use super::{
    BehaviorUse,
    RecordBuildError,
};
use crate::{
    BehaviorRegistry,
    Condition,
    ConditionBuildError,
    DefaultIds,
    MachineIds,
};

/// Borrowed condition expression containing references, never callbacks.
/// Binding enforces the native operand and depth limits before execution.
#[derive(Clone, Debug)]
pub enum ConditionRecord<'a, I: MachineIds = DefaultIds> {
    /// A trusted predicate selected by exact Story/implementation identity.
    Predicate(BehaviorUse<I::Condition, I>),
    /// All operands must hold, in declared order.
    All {
        /// Group identity.
        id: I::Condition,
        /// Borrowed operands; the native builder caps each group at 16.
        operands: &'a [Self],
    },
    /// At least one operand must hold, in declared order.
    Any {
        /// Group identity.
        id: I::Condition,
        /// Borrowed operands; the native builder caps each group at 16.
        operands: &'a [Self],
    },
    /// Negate a single expression.
    Not {
        /// Negation identity.
        id: I::Condition,
        /// Borrowed operand, subject to the native depth cap.
        operand: &'a Self,
    },
}
impl<I: MachineIds> ConditionRecord<'_, I> {
    /// Return this expression's graph identity.
    pub fn id(&self) -> &I::Condition {
        match self {
            Self::Predicate(usage) => &usage.id,
            Self::All { id, .. } | Self::Any { id, .. } | Self::Not { id, .. } => id,
        }
    }
    pub(super) fn bind<D, E, F, X, O>(
        &self,
        registry: &BehaviorRegistry<D, E, F, X, O, I>,
    ) -> Result<Condition<D, E, F, I>, RecordBuildError<I>> {
        self.bind_at(registry, 1)
    }
    fn bind_at<D, E, F, X, O>(
        &self,
        registry: &BehaviorRegistry<D, E, F, X, O, I>,
        depth: usize,
    ) -> Result<Condition<D, E, F, I>, RecordBuildError<I>> {
        // Check before descending: hostile borrowed input cannot exhaust the
        // stack.
        if depth > crate::condition::MAX_CONDITION_DEPTH {
            return Err(self.error(ConditionBuildError::DepthLimitExceeded));
        }
        match self {
            Self::Predicate(usage) => {
                Ok(registry.resolve_condition(usage.id.clone(), &usage.reference)?)
            }
            Self::Not { id, operand } => Ok(Condition::not(id.clone())
                .condition(operand.bind_at(registry, depth + 1)?)
                .map_err(|error| self.error(error))?
                .build()
                .map_err(|error| self.error(error))?),
            Self::All { id, operands } | Self::Any { id, operands } => {
                let mut builder = if matches!(self, Self::All { .. }) {
                    Condition::all(id.clone())
                } else {
                    Condition::any(id.clone())
                };
                for operand in *operands {
                    builder = builder
                        .condition(operand.bind_at(registry, depth + 1)?)
                        .map_err(|error| self.error(error))?;
                }
                Ok(builder.build().map_err(|error| self.error(error))?)
            }
        }
    }
    fn error(&self, source: ConditionBuildError) -> RecordBuildError<I> {
        RecordBuildError::Condition {
            id: self.id().clone(),
            source,
        }
    }
}
