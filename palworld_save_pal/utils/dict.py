from typing import Any, Dict


def safe_get(d: Dict[str, Any], *keys: str, default: Any = None) -> Any:
    try:
        return (
            d[keys[0]]
            if len(keys) == 1
            else safe_get(d[keys[0]], *keys[1:], default=default)
        )
    except (KeyError, TypeError, IndexError):
        return default


def safe_set(d: dict, *keys: str, value: Any) -> None:
    for key in keys[:-1]:
        if key not in d:
            raise KeyError(f"Key not found: {key}, {keys}, {d.keys()}")
        d = d[key]
    d[keys[-1]] = value


def safe_remove(d: dict, *keys: str) -> None:
    try:
        if len(keys) == 1:
            d.pop(keys[0], None)
            return
        current = d
        for key in keys[:-1]:
            current = current[key]
        current.pop(keys[-1], None)
    except (KeyError, TypeError):
        # Path doesn't exist, do nothing
        pass


def safe_remove_multiple(d: dict, *keys: str) -> None:
    for key in keys:
        safe_remove(d, key)
