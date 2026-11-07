use std::sync::Arc;

pub(crate) type EffectConstructor<D, E, F, X> =
    Arc<dyn Fn(&D, &E, &F) -> Result<X, crate::EvaluationError> + Send + Sync>;

/// A typed description of caller-owned external work.
///
/// The effect payload must match the transition's declared output type:
///
/// ```compile_fail
/// use status_machina::{Effect, EffectId, Transition, TransitionId};
/// let effect = Effect::<u64, (), (), u16>::builder(EffectId::try_new("request").unwrap())
///     .describe(|_, _, _| 1_u16).build().unwrap();
/// Transition::<u64, (), (), u8>::builder_with_effects(TransitionId::try_new("tick").unwrap())
///     .effect(effect);
/// ```
pub struct Effect<D, E, F, X, I: crate::MachineIds = crate::DefaultIds> {
    pub(crate) id: I::Effect,
    pub(crate) construct: EffectConstructor<D, E, F, X>,
    binding: Option<crate::BehaviorRef<I>>,
}

impl<D, E, F, X, I: crate::MachineIds> Effect<D, E, F, X, I> {
    /// Return the registered Story/implementation reference, if bound by a
    /// registry.
    pub const fn binding(&self) -> Option<&crate::BehaviorRef<I>> {
        self.binding.as_ref()
    }

    pub(crate) fn bound(
        id: I::Effect,
        binding: crate::BehaviorRef<I>,
        construct: EffectConstructor<D, E, F, X>,
    ) -> Self {
        Self {
            id,
            binding: Some(binding),
            construct,
        }
    }

    /// Start building a named effect intent.
    #[must_use]
    pub fn builder(id: I::Effect) -> EffectBuilder<D, E, F, X, I> {
        EffectBuilder {
            id,
            construct: None,
        }
    }
}

/// Builder for a typed effect intent.
pub struct EffectBuilder<D, E, F, X, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Effect,
    construct: Option<EffectConstructor<D, E, F, X>>,
}

impl<D, E, F, X, I: crate::MachineIds> EffectBuilder<D, E, F, X, I> {
    /// Describe the bounded request value returned to the caller after
    /// acceptance.
    #[must_use]
    pub fn describe(mut self, construct: impl Fn(&D, &E, &F) -> X + Send + Sync + 'static) -> Self {
        self.construct = Some(Arc::new(move |data, event, facts| {
            Ok(construct(data, event, facts))
        }));
        self
    }

    /// Bind an intent constructor that can return an application failure.
    #[must_use]
    pub fn try_describe(
        mut self,
        construct: impl Fn(&D, &E, &F) -> Result<X, crate::EvaluationError> + Send + Sync + 'static,
    ) -> Self {
        self.construct = Some(Arc::new(construct));
        self
    }

    /// Build the effect description.
    ///
    /// # Errors
    ///
    /// Returns an error when no effect-intent constructor was supplied.
    pub fn build(self) -> Result<Effect<D, E, F, X, I>, EffectBuildError> {
        Ok(Effect {
            binding: None,
            id: self.id,
            construct: self.construct.ok_or(EffectBuildError::MissingConstructor)?,
        })
    }
}

/// An effect builder omitted its typed intent constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EffectBuildError {
    /// No intent constructor was supplied.
    #[error("effect intent constructor is missing")]
    MissingConstructor,
}
