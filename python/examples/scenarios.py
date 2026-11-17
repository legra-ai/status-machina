"""Run with: python examples/scenarios.py (after installing status-machina).

Two scenarios replay an order machine against an independent safety assertion:
an order never ships before it is paid. Replay finds the rush transition that
skips payment and reports the bounded trace that falsified the assertion.
"""
from dataclasses import dataclass, field
from enum import Enum, auto

from status_machina import (
    Action,
    Definition,
    Event,
    Scenario,
    ScenarioAssertion,
    ScenarioLimits,
    ScenarioObservation,
    ScenarioReplay,
    ScenarioSet,
    ScenarioStep,
    State,
    Transition,
)


class Phase(Enum):
    CART = auto()
    PAID = auto()
    SHIPPED = auto()
class Input(Enum):
    PAY = auto()
    SHIP = auto()
    RUSH = auto()

@dataclass
class Order:
    paid: list[int] = field(default_factory=list)

def pay(data: Order, amount: int, facts: None) -> None:
    data.paid.append(amount)

definition: Definition[Phase, Input, Order] = Definition(id="orders", initial=Phase.CART,
    states=(State(Phase.CART), State(Phase.PAID), State(Phase.SHIPPED, terminal=True)),
    transitions=(
        Transition(id="pay", source=Phase.CART, event=Input.PAY, target=Phase.PAID,
            actions=(Action(id="record", apply=pay),)),
        Transition(id="ship", source=Phase.PAID, event=Input.SHIP, target=Phase.SHIPPED),
        Transition(id="rush", source=Phase.CART, event=Input.RUSH, target=Phase.SHIPPED),
    ))

def never_ships_unpaid(step: ScenarioObservation[str, Phase, Input]) -> bool:
    return not (step.before is Phase.CART and step.after is Phase.SHIPPED)

def run(*scenarios: Scenario[str, Input]) -> None:
    report = ScenarioReplay.run(definition, id="replay", data=Order(),
        scenarios=ScenarioSet(scenarios=scenarios, limits=ScenarioLimits(max_steps_per_scenario=8)),
        assertions=(ScenarioAssertion("never-ships-unpaid", never_ships_unpaid),))
    result = report.result
    if result.kind == "complete_within_scenarios":
        print(f"complete within {result.scenarios} scenarios, {result.nodes} steps")
    elif result.kind == "counterexample":
        print(f"counterexample to {result.trace.assertion} in {result.trace.scenario}:")
        for step in result.trace.steps:
            print(f"  {step.step}: {step.input_event.name} {step.before.name} -> {step.after.name}")

checkout = Scenario("checkout", (ScenarioStep(Event(Input.PAY, 30), None), ScenarioStep(Event(Input.SHIP, None), None)))
run(checkout)
run(checkout, Scenario("rush", (ScenarioStep(Event(Input.RUSH, None), None),)))
