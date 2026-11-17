"""Run with: python examples/composition.py (after installing status-machina)."""
from dataclasses import dataclass
from enum import Enum, auto

from status_machina import (
    Action,
    Composition,
    Definition,
    Delivery,
    Effect,
    Event,
    InstanceAddress,
    InstanceRegistry,
    OutputEvent,
    PropagationLimits,
    Route,
    State,
    Transition,
    Transitioned,
)


class Order(Enum):
    OPEN = auto()
    PAID = auto()
class OrderInput(Enum):
    PAY = auto()
class OrderOutput(Enum):
    PAID = auto()
class Shipment(Enum):
    WAITING = auto()
    PACKED = auto()
class ShipmentInput(Enum):
    PACK = auto()
class ShipmentOutput(Enum):
    PACKED = auto()
class Invoice(Enum):
    DRAFT = auto()
    ISSUED = auto()
class InvoiceInput(Enum):
    ISSUE = auto()

@dataclass(frozen=True)
class Paid:
    order: str
    amount: int

@dataclass
class Ledger:
    total: int = 0

def book(data, payload, facts):
    data.total += payload.amount

def forward(data, payload, facts):
    return payload

orders = Definition(id='orders', initial=Order.OPEN, states=(State(Order.OPEN), State(Order.PAID)),
    transitions=(Transition(id='pay', source=Order.OPEN, event=OrderInput.PAY, target=Order.PAID,
        outputs=(OutputEvent(OrderOutput.PAID, describe=forward),)),))
shipping = Definition(id='shipping', initial=Shipment.WAITING,
    states=(State(Shipment.WAITING), State(Shipment.PACKED)),
    transitions=(Transition(id='pack', source=Shipment.WAITING, event=ShipmentInput.PACK, target=Shipment.PACKED,
        effects=(Effect('print-label', describe=lambda data, payload, facts: f'label for {payload.order}'),),
        outputs=(OutputEvent(ShipmentOutput.PACKED, describe=forward),)),))
billing = Definition(id='billing', initial=Invoice.DRAFT, states=(State(Invoice.DRAFT), State(Invoice.ISSUED)),
    transitions=(Transition(id='issue', source=Invoice.DRAFT, event=InvoiceInput.ISSUE, target=Invoice.ISSUED,
        actions=(Action('book', apply=book),)),))

def main():
    # One business identity in three routing scopes: three distinct addresses.
    order = orders.instantiate(id='order-7', data=None)
    shipment = shipping.instantiate(id='order-7', data=None)
    invoice = billing.instantiate(id='order-7', data=Ledger())
    sales, warehouse, finance = (InstanceAddress(scope, 'order-7') for scope in ('sales', 'warehouse', 'finance'))
    composition = Composition(
        sources=InstanceRegistry(((sales, order), (warehouse, shipment))),
        destinations=InstanceRegistry(((warehouse, shipment), (finance, invoice))),
        routes=(Route(sales, OrderOutput.PAID, warehouse, ShipmentInput.PACK),
                Route(warehouse, ShipmentOutput.PACKED, finance, InvoiceInput.ISSUE)),
    )
    with composition.bind({sales: order, warehouse: shipment, finance: invoice}) as bound:
        report = bound.propagate([Delivery(sales, Event(OrderInput.PAY, Paid('order-7', 120)))],
            limits=PropagationLimits(max_steps=8, max_intents=8))
    # The binding is released: execute effects now, outside any machine operation.
    for step in report.steps:
        assert isinstance(step.outcome, Transitioned)
        record = step.outcome.record
        print(f'{step.destination.scope}: {record.source.name} -> {record.target.name}')
    for effect in report.effects:
        print(f'{effect.source.scope} effect: {effect.intent.payload}')
    print(report.status.name, invoice.data)

if __name__ == '__main__':
    main()
