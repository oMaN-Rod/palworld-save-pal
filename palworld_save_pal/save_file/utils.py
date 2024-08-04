from typing import Dict, Any


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
            d[key] = {}
        d = d[key]
    d[keys[-1]] = value
