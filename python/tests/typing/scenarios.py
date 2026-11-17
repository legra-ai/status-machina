"""Static contracts for scenario sets, assertions, replay and bounded reports."""
from enum import Enum, auto

from typing_extensions import assert_type

from status_machina import (
    BoundedScenarioReport,
    CounterexampleTrace,
    Definition,
    Event,
    Scenario,
    ScenarioAssertion,
    ScenarioLimit,
    ScenarioLimits,
    ScenarioObservation,
    ScenarioReplay,
    ScenarioSet,
    ScenarioStep,
    ScenarioStepKind,
    State,
    Transition,
)


class Phase(Enum):
    OPEN = auto()
    CLOSED = auto()

class Input(Enum):
    CLOSE = auto()

definition: Definition[Phase, Input, list[int]] = Definition(id="d", initial=Phase.OPEN,
    states=(State(Phase.OPEN), State(Phase.CLOSED, terminal=True)),
    transitions=(Transition(id="t", source=Phase.OPEN, event=Input.CLOSE, target=Phase.CLOSED),))
steps = (ScenarioStep(Event(Input.CLOSE, None), None),)
scenarios: ScenarioSet[str, Input] = ScenarioSet(scenarios=(Scenario("close", steps),),
    limits=ScenarioLimits(max_trace_entries=16))

def stays_open(observation: ScenarioObservation[str, Phase, Input]) -> bool:
    assert_type(observation.after, Phase)
    assert_type(observation.input_event, Input)
    assert_type(observation.outcome, ScenarioStepKind)
    return observation.after is Phase.OPEN

report = ScenarioReplay.run(definition, id="replay", data=[], scenarios=scenarios,
    assertions=(ScenarioAssertion("stays-open", stays_open),))
assert_type(report, BoundedScenarioReport[str, Phase, Input])
result = report.result
if result.kind == "counterexample":
    assert_type(result.trace, CounterexampleTrace[str, Phase, Input])
    assert_type(result.trace.steps[0].before, Phase)
elif result.kind == "limit_reached":
    assert_type(result.limit, ScenarioLimit)
    assert_type(result.scenario, str)
elif result.kind == "complete_within_scenarios":
    assert_type(result.nodes, int)

ScenarioStep(Event(Input.CLOSE, None))  # type: ignore[call-arg]
ScenarioStep((Input.CLOSE, None), None)  # type: ignore[arg-type]
ScenarioSet(scenarios=(Scenario("close", steps),), limits={"max_nodes": 4})  # type: ignore[arg-type]
ScenarioLimits(max_nodes="4")  # type: ignore[arg-type]
ScenarioLimits(4)  # type: ignore[call-arg]
ScenarioAssertion("stays-open", lambda observation: "open")  # type: ignore[arg-type, return-value]
ScenarioReplay.run(definition, id="replay", data=[], scenarios=steps)  # type: ignore[arg-type]
ScenarioReplay.run(definition, id="replay", data="text", scenarios=scenarios)  # type: ignore[misc]
ScenarioReplay.run(definition, data=[], scenarios=scenarios)  # type: ignore[call-arg]
