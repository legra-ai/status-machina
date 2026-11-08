"""Consume only the declared finite number of native specification items."""
from .errors import LimitError

def bounded(values, limit, label):
    if type(limit) is not int or not 1 <= limit <= 65536:
        raise LimitError(f"{label} limit must be an integer between 1 and 65536")
    result = []
    for value in values:
        if len(result) == limit:
            raise LimitError(f"{label} exceeds its declared limit", limit=limit)
        result.append(value)
    return tuple(result)
