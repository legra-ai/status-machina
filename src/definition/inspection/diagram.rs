use super::{
    InspectionError,
    InspectionLimits,
    MachineDefinition,
};
use crate::TransitionKind;

struct Diagram {
    text: String,
    limit: usize,
}

impl Diagram {
    fn append(&mut self, text: &str) -> Result<(), InspectionError> {
        if text.len() > self.limit.saturating_sub(self.text.len()) {
            return Err(InspectionError::OutputLimit { limit: self.limit });
        }
        self.text.push_str(text);
        Ok(())
    }

    fn label(&mut self, text: &str) -> Result<(), InspectionError> {
        for character in text.chars() {
            if character.is_alphanumeric() || matches!(character, ' ' | '-' | '_' | '.') {
                self.append(character.encode_utf8(&mut [0; 4]))?;
            } else {
                self.append(&format!("#{};", u32::from(character)))?;
            }
        }
        Ok(())
    }
}

impl<D, E, F, X, O, I: crate::MachineIds> MachineDefinition<D, E, F, X, O, I>
where
    I::State: Ord,
    I::Transition: Ord,
    I::Disposition: Ord,
{
    /// Render a canonical Mermaid state diagram from the validated definition.
    ///
    /// Labels include declared guard identities and dispositions without
    /// executing them. Sorting by identifier makes output independent of
    /// declaration order.
    ///
    /// # Errors
    /// Rejects an oversized graph or output before exceeding the configured
    /// bound.
    pub fn diagram_with_labels(
        &self,
        limits: &InspectionLimits,
        labels: &impl crate::DiagramLabels<I>,
    ) -> Result<String, InspectionError> {
        self.check_inspection_limit(limits)?;
        // bounded: graph entries were checked against the inspection limit.
        let mut states: Vec<_> = self.states.iter().collect();
        states.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        // bounded: graph entries were checked against the inspection limit.
        let mut transitions: Vec<_> = self.transitions.iter().collect();
        transitions.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        // bounded: graph entries were checked against the inspection limit.
        let mut dispositions: Vec<_> = self.dispositions.iter().collect();
        dispositions.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        let index = |id: &I::State| {
            states
                .iter()
                .position(|state| state.id == *id)
                .ok_or(InspectionError::MissingGraphEndpoint)
        };
        let mut output = Diagram {
            text: String::new(),
            limit: limits.max_output_bytes,
        };
        output.append("stateDiagram-v2\n")?;
        for (number, state) in states.iter().enumerate() {
            output.append("    state \"")?;
            output.label(labels.state(&state.id))?;
            output.append(&format!("\" as s{number}\n"))?;
            if state.terminal() {
                output.append(&format!("    s{number} --> [*]\n"))?;
            }
        }
        output.append(&format!("    [*] --> s{}\n", index(&self.initial)?))?;
        for edge in transitions {
            output.append(&format!(
                "    s{} --> s{} : ",
                index(&edge.from)?,
                index(&edge.to)?
            ))?;
            output.label(labels.transition(&edge.id))?;
            output.append(" / ")?;
            output.label(labels.event(&edge.event))?;
            if let Some(guard) = &edge.guard {
                output.append(" [guard: ")?;
                output.label(labels.condition(guard.id()))?;
                output.append("]")?;
            }
            output.append(match edge.kind {
                TransitionKind::Internal => " (internal)\n",
                TransitionKind::External => " (external)\n",
            })?;
        }
        for rule in dispositions {
            output.append(&format!(
                "    s{} --> s{} : disposition: ",
                index(&rule.from)?,
                index(&rule.from)?
            ))?;
            output.label(labels.disposition(&rule.id))?;
            output.append(" / ")?;
            output.label(labels.event(&rule.event))?;
            output.append(" [guard: ")?;
            output.label(labels.condition(rule.condition.id()))?;
            output.append("] ")?;
            output.label(&rule.reason)?;
            output.append("\n")?;
        }
        Ok(output.text)
    }
}

impl<D, E, F, X, O> MachineDefinition<D, E, F, X, O> {
    /// Render the default family's validated text identifiers as diagram
    /// labels.
    ///
    /// # Errors
    /// Rejects an oversized graph or output before exceeding the configured
    /// bound.
    pub fn diagram(&self, limits: &InspectionLimits) -> Result<String, InspectionError> {
        self.diagram_with_labels(limits, &crate::DefaultIds)
    }
}
