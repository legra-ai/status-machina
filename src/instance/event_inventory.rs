use super::MachineInstance;

impl<'definition, D, E, F, X, O, I: crate::MachineIds>
    MachineInstance<'definition, D, E, F, X, O, I>
where
    D: crate::FallibleCandidateData,
{
    pub(crate) fn input_event_ids(&self) -> impl Iterator<Item = &I::Event> {
        self.definition()
            .transitions()
            .iter()
            .map(|transition| &transition.event)
            .chain(
                self.definition()
                    .dispositions()
                    .iter()
                    .map(|disposition| &disposition.event),
            )
    }

    pub(crate) fn output_event_ids(&self) -> impl Iterator<Item = &I::Event> {
        self.definition()
            .transitions()
            .iter()
            .flat_map(|transition| {
                transition
                    .output_events
                    .iter()
                    .map(|output_event| &output_event.id)
            })
    }
}
