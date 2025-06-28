# Palworld Save Pal API/Message Documentation

This document describes the WebSocket message types and their payloads shared between the backend (FastAPI) and frontend (Svelte).

## Message Envelope

All messages follow this structure:

```
{
  "type": "MessageType",
  "data": { ...payload }
}
```

- `type`: One of the `MessageType` enum values.
- `data`: The payload for the message (see below).

## Message Types

| Type | Payload | Description |
|------|---------|-------------|
| add_pal | { ... } | Add a Pal to the save file |
| delete_pals | { ... } | Delete Pals by ID |
| get_guilds | null | Get all guilds |
| ... | ... | ... |

> See `shared/models.py` and `shared/types.ts` for the full list and structure of payloads.

## Adding/Updating Messages

1. Update or add the Pydantic model in `shared/models.py`.
2. Regenerate `shared/types.ts` for TypeScript types.
3. Update this documentation table as needed.

---

## Example

```
{
  "type": "add_pal",
  "data": {
    "pal_id": "...",
    "player_id": "..."
  }
}
```

---

## TODO
- [ ] Document all message payloads in detail.
- [ ] Add example requests and responses for each message type.
