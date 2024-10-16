import sys
from pathlib import Path
import multiprocessing
import threading
import time
import webview
import uvicorn
from fastapi import FastAPI, Request, WebSocket, WebSocketDisconnect
from fastapi.responses import FileResponse
import psutil
import argparse

from palworld_save_pal.ws.manager import ConnectionManager
from palworld_save_pal.utils.logging_config import create_logger, setup_logging
from palworld_save_pal.__version__ import __version__

logger = create_logger(__name__)

app = FastAPI(swagger_ui_parameters={"syntaxHighlight.theme": "monokai"})
manager = ConnectionManager()


class AppState:
    def __init__(self):
        self.terminate_flag = threading.Event()
        self.server_instance = None
        self.webview_window = None


app_state = AppState()


@app.middleware("http")
async def static_files_middleware(request: Request, call_next):
    path = request.url.path
    if path.startswith("/ws"):
        response = await call_next(request)
        return response

    file_path = Path("build") / path.lstrip("/")
    if file_path.is_dir():
        index_path = file_path / "index.html"
        if index_path.is_file():
            return FileResponse(index_path)
    elif file_path.is_file():
        return FileResponse(file_path)
    return await call_next(request)


@app.websocket("/ws/{client_id}")
async def websocket_endpoint(websocket: WebSocket, client_id: int):
    logger.info("Client %s connected", client_id)
    await manager.connect(websocket)
    try:
        while not app_state.terminate_flag.is_set():
            data = await websocket.receive_text()
            await manager.process_message(data, websocket)
    except WebSocketDisconnect:
        logger.warning("Client %s disconnected", client_id)
        manager.disconnect(websocket)


class ServerThread(threading.Thread):
    def __init__(self, host, port, dev_mode):
        super().__init__()
        self.host = host
        self.port = port
        self.dev_mode = dev_mode
        self.server = None

    def run(self):
        logger.debug("Starting server thread")
        config = uvicorn.Config(
            app=app,
            host=self.host,
            port=self.port,
            reload=self.dev_mode,
            ws_max_size=2**30,  # 1 GB limit
        )
        self.server = uvicorn.Server(config)
        self.server.run()

    def stop(self):
        if self.server:
            logger.debug("Stopping server")
            self.server.should_exit = True
        else:
            logger.warning("Server instance not found during stop attempt")


def cleanup_processes():
    logger.debug("Starting process cleanup")
    current_process = psutil.Process()
    children = current_process.children(recursive=True)
    for child in children:
        child.terminate()
    _, alive = psutil.wait_procs(children, timeout=5)
    for p in alive:
        logger.warning("Force killing process: %s", p.pid)
        p.kill()
    logger.debug("Process cleanup completed")


def start_server(host, port, dev_mode):
    logger.info("Starting server on %s:%s (dev mode: %s)", host, port, dev_mode)
    app_state.server_instance = ServerThread(host, port, dev_mode)
    app_state.server_instance.start()


def start_webview(url, debug):
    logger.info("Starting webview with URL: %s", url)
    webview.settings["ALLOW_DOWNLOADS"] = True
    app_state.webview_window = webview.create_window(
        f"Palworld Save Pal v{__version__}",
        url,
        width=1366,
        height=768,
        min_size=(1366, 768),
    )
    app_state.webview_window.events.closed += on_closed
    webview.start(debug=debug, user_agent="pywebview")


def on_closed():
    logger.info("Webview window closed. Initiating shutdown...")
    app_state.terminate_flag.set()
    if app_state.server_instance:
        app_state.server_instance.stop()
    cleanup_processes()
    logger.info("Shutdown process completed")
    sys.exit(0)


def parse_arguments():
    parser = argparse.ArgumentParser(
        description="Start Palworld Save Pal Desktop Application"
    )
    parser.add_argument("--dev", action="store_true", help="Run in development mode")
    parser.add_argument(
        "--port", default=5174, type=int, help="Port to run the server on"
    )
    parser.add_argument("--host", default="127.0.0.1", help="Host to run the server on")
    return parser.parse_args()


def main():
    multiprocessing.freeze_support()
    args = parse_arguments()
    setup_logging(dev_mode=args.dev)
    global logger
    logger = create_logger(__name__)

    logger.info(
        "Starting application in %s mode on %s:%s",
        "development" if args.dev else "production",
        args.host,
        args.port,
    )

    start_server(args.host, args.port, args.dev)

    time.sleep(2)
    url = f"http://{args.host}:{args.port}"
    start_webview(url, args.dev)

    logger.debug("Main thread waiting for termination signal")
    app_state.terminate_flag.wait()

    logger.debug("Termination signal received, initiating shutdown")
    if app_state.server_instance:
        app_state.server_instance.stop()
    cleanup_processes()
    logger.info("Application shutdown complete, goodbye!")


if __name__ == "__main__":
    main()
