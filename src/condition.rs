mod bindings;

use std::sync::Arc;

const MAX_CONDITION_OPERANDS: usize = 16;
pub(crate) const MAX_CONDITION_DEPTH: usize = 32;

pub(crate) type Predicate<D, E, F> = Arc<dyn Fn(&D, &E, &F) -> bool + Send + Sync>;

/// Shape of a condition expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionKind {
    /// One caller-bound predicate.
    Predicate,
    /// Every child condition must hold.
    All,
    /// At least one child condition must hold.
    Any,
    /// The single child condition must be false.
    Not,
}

enum ConditionBody<D, E, F, I: crate::MachineIds = crate::DefaultIds> {
    Predicate(Predicate<D, E, F>),
    All(Vec<Condition<D, E, F, I>>),
    Any(Vec<Condition<D, E, F, I>>),
    Not(Box<Condition<D, E, F, I>>),
}

/// Named, bounded condition expression used by guards and postconditions.
pub struct Condition<D, E, F, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Condition,
    body: ConditionBody<D, E, F, I>,
    depth: usize,
    binding: Option<crate::BehaviorRef<I>>,
}

impl<D, E, F, I: crate::MachineIds> Condition<D, E, F, I> {
    /// Return this predicate's registered Story/implementation reference.
    pub const fn binding(&self) -> Option<&crate::BehaviorRef<I>> {
        self.binding.as_ref()
    }

    pub(crate) fn bound(
        id: I::Condition,
        binding: crate::BehaviorRef<I>,
        predicate: Predicate<D, E, F>,
    ) -> Self {
        Self {
            id,
            body: ConditionBody::Predicate(predicate),
            depth: 1,
            binding: Some(binding),
        }
    }

    /// Start a builder for an atomic predicate condition.
    #[must_use]
    pub fn builder(id: I::Condition) -> ConditionBuilder<D, E, F, I> {
        ConditionBuilder {
            id,
            predicate: None,
        }
    }

    /// Start a builder that requires all child conditions.
    #[must_use]
    pub fn all(id: I::Condition) -> ConditionGroupBuilder<D, E, F, I> {
        ConditionGroupBuilder::new(id, ConditionKind::All)
    }

    /// Start a builder that requires at least one child condition.
    #[must_use]
    pub fn any(id: I::Condition) -> ConditionGroupBuilder<D, E, F, I> {
        ConditionGroupBuilder::new(id, ConditionKind::Any)
    }

    /// Start a builder that negates one child condition.
    #[must_use]
    pub fn not(id: I::Condition) -> ConditionNotBuilder<D, E, F, I> {
        ConditionNotBuilder { id, child: None }
    }

    /// Return this condition's stable identifier.
    #[must_use]
    pub const fn id(&self) -> &I::Condition {
        &self.id
    }

    /// Return the structural condition kind.
    #[must_use]
    pub const fn kind(&self) -> ConditionKind {
        match &self.body {
            ConditionBody::Predicate(_) => ConditionKind::Predicate,
            ConditionBody::All(_) => ConditionKind::All,
            ConditionBody::Any(_) => ConditionKind::Any,
            ConditionBody::Not(_) => ConditionKind::Not,
        }
    }

    pub(crate) fn evaluate(&self, data: &D, event: &E, facts: &F) -> bool {
        match &self.body {
            ConditionBody::Predicate(predicate) => predicate(data, event, facts),
            ConditionBody::All(children) => children
                .iter()
                .all(|child| child.evaluate(data, event, facts)),
            ConditionBody::Any(children) => children
                .iter()
                .any(|child| child.evaluate(data, event, facts)),
            ConditionBody::Not(child) => !child.evaluate(data, event, facts),
        }
    }
}

/// Builder for an atomic named condition.
pub struct ConditionBuilder<D, E, F, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Condition,
    predicate: Option<Predicate<D, E, F>>,
}

impl<D, E, F, I: crate::MachineIds> ConditionBuilder<D, E, F, I> {
    /// Bind the condition's pure predicate.
    #[must_use]
    pub fn evaluate(
        mut self,
        predicate: impl Fn(&D, &E, &F) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.predicate = Some(Arc::new(predicate));
        self
    }

    /// Build the atomic condition.
    ///
    /// # Errors
    ///
    /// Returns an error when no predicate implementation was supplied.
    pub fn build(self) -> Result<Condition<D, E, F, I>, ConditionBuildError> {
        Ok(Condition {
            id: self.id,
            body: ConditionBody::Predicate(
                self.predicate
                    .ok_or(ConditionBuildError::MissingPredicate)?,
            ),
            depth: 1,
            binding: None,
        })
    }
}

/// Builder for an `all` or `any` condition group.
pub struct ConditionGroupBuilder<D, E, F, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Condition,
    kind: ConditionKind,
    children: Vec<Condition<D, E, F, I>>,
    depth: usize,
}

/// Builder for one negated condition.
pub struct ConditionNotBuilder<D, E, F, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Condition,
    child: Option<Condition<D, E, F, I>>,
}

impl<D, E, F, I: crate::MachineIds> ConditionNotBuilder<D, E, F, I> {
    /// Set the child condition.
    ///
    /// # Errors
    ///
    /// Returns an error if a child was already provided or nesting would exceed
    /// 32.
    pub fn condition(
        mut self,
        condition: Condition<D, E, F, I>,
    ) -> Result<Self, ConditionBuildError> {
        if self.child.is_some() {
            return Err(ConditionBuildError::DuplicateOperand);
        }
        if condition.depth + 1 > MAX_CONDITION_DEPTH {
            return Err(ConditionBuildError::DepthLimitExceeded);
        }
        self.child = Some(condition);
        Ok(self)
    }

    /// Build the negation.
    ///
    /// # Errors
    ///
    /// Returns an error when no child condition was supplied.
    pub fn build(self) -> Result<Condition<D, E, F, I>, ConditionBuildError> {
        let child = self.child.ok_or(ConditionBuildError::EmptyNegation)?;
        let depth = child.depth + 1;
        Ok(Condition {
            id: self.id,
            body: ConditionBody::Not(Box::new(child)),
            depth,
            binding: None,
        })
    }
}

impl<D, E, F, I: crate::MachineIds> ConditionGroupBuilder<D, E, F, I> {
    fn new(id: I::Condition, kind: ConditionKind) -> Self {
        Self {
            id,
            kind,
            children: Vec::new(),
            depth: 1,
        }
    }

    /// Add one bounded child condition.
    ///
    /// # Errors
    ///
    /// Returns an error when the operand or nesting-depth limit is exceeded.
    pub fn condition(
        mut self,
        condition: Condition<D, E, F, I>,
    ) -> Result<Self, ConditionBuildError> {
        if self.children.len() == MAX_CONDITION_OPERANDS {
            return Err(ConditionBuildError::OperandLimitExceeded);
        }
        let depth = condition.depth + 1;
        if depth > MAX_CONDITION_DEPTH {
            return Err(ConditionBuildError::DepthLimitExceeded);
        }
        self.depth = self.depth.max(depth);
        self.children.push(condition);
        Ok(self)
    }

    /// Build the bounded condition group.
    ///
    /// # Errors
    ///
    /// Returns an error when the group has no child conditions.
    pub fn build(self) -> Result<Condition<D, E, F, I>, ConditionBuildError> {
        if self.children.is_empty() {
            return Err(ConditionBuildError::EmptyGroup);
        }
        let body = match self.kind {
            ConditionKind::All => ConditionBody::All(self.children),
            ConditionKind::Any => ConditionBody::Any(self.children),
            ConditionKind::Predicate | ConditionKind::Not => {
                return Err(ConditionBuildError::InvalidGroupKind);
            }
        };
        Ok(Condition {
            id: self.id,
            body,
            depth: self.depth,
            binding: None,
        })
    }
}

/// Invalid condition structure or configured bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ConditionBuildError {
    /// An atomic condition has no predicate.
    #[error("condition predicate is missing")]
    MissingPredicate,
    /// A group has no child conditions.
    #[error("condition group must contain at least one child")]
    EmptyGroup,
    /// A condition group exceeded its operand bound.
    #[error("condition group exceeds 16 operands")]
    OperandLimitExceeded,
    /// A condition tree exceeded its nesting bound.
    #[error("condition nesting exceeds 32 levels")]
    DepthLimitExceeded,
    /// Predicate conditions cannot be built as groups.
    #[error("atomic predicate is not a group condition")]
    InvalidGroupKind,
    /// A negation has no child condition.
    #[error("condition negation requires one child")]
    EmptyNegation,
    /// An operand was supplied more than once to a singular builder.
    #[error("condition builder already has an operand")]
    DuplicateOperand,
}
