"""Static contracts for outcomes, decisions, Story references, errors and EventOnDrop."""
from enum import Enum, auto

from typing_extensions import assert_type

from status_machina import (
    BehaviorRef,
    BindingError,
    Condition,
    Decision,
    Definition,
    DefinitionError,
    DefinitionLimitError,
    Disposition,
    Event,
    EventOnDrop,
    Ignored,
    LimitError,
    Refused,
    State,
    Transition,
    Transitioned,
    TransitionRecord,
    UnknownBehaviorError,
)


class Phase(Enum):
    OPEN = auto()
    CLOSED = auto()

class Input(Enum):
    CLOSE = auto()
    PING = auto()

definition = Definition[Phase, Input, list[int]](id="d", initial=Phase.OPEN,
    states=(State(Phase.OPEN), State(Phase.CLOSED, terminal=True)),
    transitions=(Transition(id="t", source=Phase.OPEN, event=Input.CLOSE, target=Phase.CLOSED,
        guard=Condition("g", all_of=(Condition("a", lambda data, payload, facts: True),))),),
    dispositions=(Disposition("p", Phase.OPEN, Input.PING, Condition("c", lambda data, payload, facts: True), "dup"),))
machine = definition.instantiate(id="m", data=[])
outcome = machine.dispatch(Event(Input.CLOSE, None), expected_revision=0)
if outcome.kind == "transitioned":
    assert_type(outcome, Transitioned[Phase])
    assert_type(outcome.record, TransitionRecord[Phase])
    assert_type(outcome.record.target, Phase)
elif outcome.kind == "ignored":
    assert_type(outcome, Ignored[Phase, Input])
    assert_type(outcome.reason, str)
else:
    assert_type(outcome, Refused[Phase, Input])
    assert_type(outcome.event, Input)
decision = machine.evaluate(Event(Input.CLOSE, None))
assert_type(decision, Decision)
assert_type(decision.expected_revision, int)
reference = BehaviorRef("story", "v1")
error: DefinitionError = UnknownBehaviorError("missing")
binding: BindingError = UnknownBehaviorError("missing")
limit: LimitError = DefinitionLimitError("too many")

guard = EventOnDrop(Event(Input.CLOSE, None), lambda event: None)
assert_type(guard, EventOnDrop[Event[Input, None]])
assert_type(guard.event, Event[Input, None] | None)
with guard as scope:
    assert_type(scope, EventOnDrop[Event[Input, None]])
    scope.disarm()

machine.dispatch(Event(Input.CLOSE, None), expected_revision="0")  # type: ignore[arg-type]
machine.accept(outcome)  # type: ignore[arg-type]
wrong: Transitioned[str] = outcome  # type: ignore[assignment]
guard.event = "abandoned"  # type: ignore[assignment]
EventOnDrop(Event(Input.CLOSE, None), lambda event, extra: None)  # type: ignore[arg-type, misc]
