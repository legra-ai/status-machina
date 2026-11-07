use super::{
    Condition,
    ConditionBody,
};

struct Bindings<'a, D, E, F, I: crate::MachineIds> {
    // bounded: DFS frontier <= 1 + (16 - 1) * 31, from condition depth/operand
    // caps.
    pending: Vec<&'a Condition<D, E, F, I>>,
}
impl<D, E, F, I: crate::MachineIds> Condition<D, E, F, I> {
    pub(crate) fn binding_refs(&self) -> impl Iterator<Item = &crate::BehaviorRef<I>> {
        Bindings {
            pending: vec![self],
        }
    }
}
impl<'a, D, E, F, I: crate::MachineIds> Iterator for Bindings<'a, D, E, F, I> {
    type Item = &'a crate::BehaviorRef<I>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(condition) = self.pending.pop() {
            match &condition.body {
                ConditionBody::Predicate(_) => {}
                ConditionBody::All(children) | ConditionBody::Any(children) => {
                    self.pending.extend(children.iter().rev())
                }
                ConditionBody::Not(child) => self.pending.push(child),
            }
            if let Some(binding) = condition.binding() {
                return Some(binding);
            }
        }
        None
    }
}
