"""Run with: python examples/order.py (after installing status-machina)."""
from dataclasses import dataclass
from enum import Enum, auto
from status_machina import Action, Condition, Definition, DiagramLabels, Event, State, Transition

class Phase(Enum):
    PENDING = auto()
    APPROVED = auto()
class Input(Enum):
    APPROVE = auto()
class Step(Enum):
    APPROVE = auto()
class Policy(Enum):
    POSITIVE_TOTAL = auto()
class Change(Enum):
    MARK_APPROVED = auto()

@dataclass
class Order:
    total: int
    approved: bool = False

def mark_approved(data, payload, facts):
    data.approved = True

def definition():
    return Definition(id='Order approval', initial=Phase.PENDING,
        states=(State(Phase.PENDING), State(Phase.APPROVED, terminal=True)),
        transitions=(Transition(id=Step.APPROVE, source=Phase.PENDING, event=Input.APPROVE,
            target=Phase.APPROVED, guard=Condition(Policy.POSITIVE_TOTAL, predicate=lambda data, payload, facts: data.total > 0),
            actions=(Action(Change.MARK_APPROVED, apply=mark_approved),)),))

def diagram():
    return definition().diagram(DiagramLabels(
        states={Phase.PENDING: 'Pending', Phase.APPROVED: 'Approved'},
        transitions={Step.APPROVE: 'Approve order'}, events={Input.APPROVE: 'Approve'},
        conditions={Policy.POSITIVE_TOTAL: 'Positive total'},
    ))

def main():
    graph = definition()
    with graph.instantiate(id=object(), data=Order(120)) as machine:
        outcome = machine.dispatch(Event(Input.APPROVE, None), facts=None)
        assert outcome.kind == 'transitioned'
        assert machine.state is Phase.APPROVED
        assert machine.data == Order(120, approved=True)
    print(diagram(), end='')

if __name__ == '__main__':
    main()
