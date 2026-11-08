"""Static contracts checked with mypy --warn-unused-ignores."""
from enum import Enum, auto
from typing_extensions import assert_type
from status_machina import DiagramLabels, InspectionLimits, StructuralCoverage, Action, Capability, CapabilityReport, CapabilityStatus, Definition, Event, Machine, State, Transition

class Phase(Enum):
    READY = auto()
    DONE = auto()

class Input(Enum):
    FINISH = auto()

class Wrong(Enum):
    FINISH = auto()

def finish(data: int | None, payload: object, facts: object) -> int | None:
    return None

definition = Definition[Phase, Input, int | None](
    id=object(), initial=Phase.READY,
    states=(State(Phase.READY), State(Phase.DONE, terminal=True)),
    transitions=(Transition(id=object(), source=Phase.READY, event=Input.FINISH,
        target=Phase.DONE, actions=(Action(id=object(), update=finish),)),),
)
machine = definition.instantiate(id=object(), data=1)
assert_type(machine, Machine[Phase, Input, int | None])
assert_type(machine.state, Phase)
assert_type(machine.data, int | None)
assert_type(machine.revision, int)
machine.dispatch(Event(Input.FINISH, None), facts=None)
machine.dispatch(Event(Wrong.FINISH, None), facts=None)  # type: ignore[arg-type]
definition.instantiate(id=object(), data="wrong")  # type: ignore[arg-type]
wrong_state: str = machine.state  # type: ignore[assignment]
wrong_data: int = machine.data  # type: ignore[assignment]

policy = Capability[Phase, int | None, object](id=object(),
    evaluate=lambda state, data, facts: CapabilityStatus.UNKNOWN)
assert_type(machine.evaluate_capability(policy.id, facts=None), CapabilityReport)

coverage = definition.structural_coverage(limits=InspectionLimits(max_entries=10))
assert_type(coverage, StructuralCoverage[Phase])
assert_type(coverage.states[0].id, Phase)
labels = DiagramLabels[Phase, Input](states={Phase.READY: "Ready", Phase.DONE: "Done"})
assert_type(definition.diagram(labels), str)
wrong_labels = DiagramLabels[Wrong, Input]()
definition.diagram(wrong_labels)  # type: ignore[arg-type]
