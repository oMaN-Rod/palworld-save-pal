# Shared Types and DTOs

This directory contains shared data models and message types for both backend (Python) and frontend (TypeScript).

## How to Use

- **Python:** Import Pydantic models from this directory for FastAPI message validation.
- **TypeScript:** Use generated types for strict typing of WebSocket messages and payloads.

## Code Generation

- Use `pydantic2ts` or `datamodel-code-generator` to generate TypeScript types from Python Pydantic models.
- Regenerate types after changing backend models.

## Message Types

- All WebSocket message types and payloads should be defined here for consistency.

---

## Example Workflow

1. Define or update Pydantic models in `shared/models.py`.
2. Run the codegen script (`generate-types.ps1`) to update `shared/types.ts`.
3. Import and use the types in both backend and frontend.

---

## TODO
- [x] Move all message/data models from backend to `shared/models.py`.
- [x] Set up codegen for TypeScript types.
- [ ] Update frontend to use generated types.
- [ ] Document all message types and payloads here.

---

## Message Types Documentation

| MessageType              | Payload Model              | Description                       |
|-------------------------|---------------------------|-----------------------------------|
| DOWNLOAD_SAVE_FILE      | DownloadSaveFileMessage   | Download a save file              |
| LOAD_ZIP_FILE           | LoadZipFileMessage        | Load a zip file                   |
| UPDATE_SAVE_FILE        | UpdateSaveFileMessage     | Update a save file                |
| SYNC_APP_STATE          | SyncAppStateMessage       | Sync application state            |
| DELETE_PALS             | DeletePalsMessage         | Delete pals                       |
| ...                     | ...                       | ...                               |

Add details for each message type and its fields as you expand the models.
