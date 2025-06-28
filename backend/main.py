# FastAPI app entrypoint
from fastapi import FastAPI, WebSocket
from palworld_save_pal.ws.manager import ConnectionManager
from sqlmodel import SQLModel, create_engine
import os

app = FastAPI()
manager = ConnectionManager()

def get_db_url():
    # Use the same DB path as the rest of the app
    db_path = os.environ.get("PALWORLD_DB_PATH", "psp.db")
    return f"sqlite:///{db_path}"

@app.on_event("startup")
def on_startup():
    engine = create_engine(get_db_url())
    SQLModel.metadata.create_all(engine)

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await manager.connect(websocket)
    try:
        while True:
            data = await websocket.receive_text()
            await manager.process_message(data, websocket)
    except Exception:
        pass
    finally:
        manager.disconnect(websocket)
