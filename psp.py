import argparse
from pathlib import Path
import multiprocessing
from urllib.parse import quote

import uvicorn
from fastapi import FastAPI, Request, WebSocket, WebSocketDisconnect
from fastapi.responses import FileResponse, RedirectResponse
from palworld_save_pal.db.bootstrap import create_db_and_tables
from palworld_save_pal.ws.manager import ConnectionManager

from palworld_save_pal.utils.logging_config import create_logger, setup_logging

PORT = 5174

logger = create_logger(__name__)


# Initialize the FastAPI app
app = FastAPI(swagger_ui_parameters={"syntaxHighlight.theme": "monokai"})

manager = ConnectionManager()


@app.middleware("http")
async def static_files_middleware(request: Request, call_next):
    path = request.url.path
    if path.startswith("/ws"):
        response = await call_next(request)
        return response

    file_path = Path("ui") / path.lstrip("/")
    if file_path.is_dir():
        index_path = file_path / "index.html"
        if index_path.is_file():
            return FileResponse(index_path)
    elif file_path.is_file():
        return FileResponse(file_path)

    # If no static file matches the requested path, redirect to the root path with the
    # original URL as a query parameter. This is to handle client-side routing in the SPA.
    if path != "/":
        encoded_path = quote(path)
        return RedirectResponse(url=f"/?path={encoded_path}")
    return await call_next(request)


@app.websocket("/ws/{client_id}")
async def websocket_endpoint(websocket: WebSocket, client_id: int):
    logger.info("Client %s connected", client_id)
    await manager.connect(websocket)
    try:
        while True:
            data = await websocket.receive_text()
            await manager.process_message(data, websocket)
    except WebSocketDisconnect:
        logger.warning("Client %s disconnected", client_id)
        manager.disconnect(websocket)


def parse_arguments():
    parser = argparse.ArgumentParser(description="Start Palworld Save Pal")
    parser.add_argument("--dev", action="store_true", help="Run in development mode")
    parser.add_argument(
        "--port", default=5174, type=int, help="Port to run the server on"
    )
    parser.add_argument("--host", default="0.0.0.0", help="Host to run the server on")
    return parser.parse_args()


if __name__ == "__main__":
    multiprocessing.freeze_support()
    args = parse_arguments()
    create_db_and_tables()
    setup_logging(dev_mode=args.dev)
    logger = create_logger(__name__)
    DEV_MODE = args.dev
    PORT = args.port
    HOST = args.host

    logger.info(
        "Starting server in %s mode on %s:%s",
        "development" if DEV_MODE else "production",
        HOST,
        PORT,
    )

    config = uvicorn.Config(
        f"{Path(__file__).stem}:app",
        host=HOST,
        port=PORT,
        reload=True if DEV_MODE else False,
        ws_max_size=2**30,  # 1 GB limit
    )
    server = uvicorn.Server(config)
    server.run()
