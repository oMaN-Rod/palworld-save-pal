import json
import os
import platform
import signal
import subprocess
import traceback
from urllib.parse import quote
import sys
from pathlib import Path
import multiprocessing
import time
import webview
from fastapi import FastAPI, Request, WebSocket, WebSocketDisconnect
from fastapi.responses import FileResponse, RedirectResponse
import psutil
import argparse
from typing import Any

from palworld_save_pal.db.bootstrap import create_db_and_tables
from palworld_save_pal.server_thread import ServerThread
from palworld_save_pal.utils.file_manager import FileManager, STEAM_ROOT, GAMEPASS_ROOT
from palworld_save_pal.ws.manager import ConnectionManager
from palworld_save_pal.utils.logging_config import create_logger, setup_logging
from palworld_save_pal.__version__ import __version__
from palworld_save_pal.ws.messages import MessageType
from palworld_save_pal.ws.utils import build_response
from palworld_save_pal.state import get_app_state

logger = create_logger(__name__)

app = FastAPI(swagger_ui_parameters={"syntaxHighlight.theme": "monokai"})
manager = ConnectionManager()

app_state = get_app_state()


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


def get_app_root() -> str:
    if getattr(sys, "frozen", False):
        return os.path.dirname(sys.executable)
    return os.path.dirname(os.path.abspath(__file__))


def get_folder_path(folder_type: str) -> str | None:
    app_root = get_app_root()
    folder_paths = {
        "backups": os.path.join(app_root, "backups"),
        "steam": STEAM_ROOT,
        "gamepass": GAMEPASS_ROOT,
        "psp_root": app_root,
    }
    return folder_paths.get(folder_type)


def open_folder_in_explorer(folder_path: str) -> None:
    system = platform.system()
    try:
        if system == "Windows":
            os.startfile(folder_path)
        elif system == "Darwin":
            subprocess.run(["open", folder_path], check=True)
        else:
            subprocess.run(["xdg-open", folder_path], check=True)
        logger.info("Opened folder: %s", folder_path)
    except Exception as e:
        logger.error("Failed to open folder %s: %s", folder_path, str(e))


async def handle_file_selection(
    save_type: str, window: webview.Window, websocket: WebSocket
) -> tuple[str | None, str | None]:
    try:
        result = FileManager.open_file_dialog(
            save_type, window, app_state.settings.save_dir
        )
    except Exception as e:
        logger.error("Error opening file dialog: %s", str(e))
        traceback.print_exc()
        response = build_response(
            MessageType.ERROR,
            {
                "message": str(e),
                "trace": traceback.format_exc(),
            },
        )
        await websocket.send_json(response)
        return None, None
    if not result:
        response = build_response(MessageType.NO_FILE_SELECTED, "No file selected")
        await websocket.send_json(response)
        return None, None
    save_dir = str(Path(result).parent)
    return save_dir, result


@app.websocket("/ws/{client_id}")
async def websocket_endpoint(websocket: WebSocket, client_id: int):
    logger.info("Client %s connected", client_id)
    await manager.connect(websocket)
    try:
        while not app_state.terminate_flag.is_set():
            data = await websocket.receive_text()
            json_data = json.loads(data)
            match json_data.get("type"):
                case "select_save":
                    save_type = json_data["data"]["type"]
                    save_dir, file_path = await handle_file_selection(
                        save_type, app_state.webview_window, websocket
                    )
                    if not save_dir or not file_path:
                        continue
                    app_state.settings.save_dir = save_dir
                    json_data["data"]["path"] = file_path
                    data = json.dumps(json_data)
                case "unlock_map":
                    save_dir, file_path = await handle_file_selection(
                        "local_data", app_state.webview_window, websocket
                    )
                    if not save_dir or not file_path:
                        continue
                    json_data["data"]["path"] = file_path
                    data = json.dumps(json_data)
                case "open_folder":
                    folder_type = json_data.get("data", {}).get("folder_type", "")
                    folder_path = get_folder_path(folder_type)
                    if folder_path and os.path.exists(folder_path):
                        open_folder_in_explorer(folder_path)
                    else:
                        response = build_response(
                            MessageType.WARNING,
                            f"Folder not found: {folder_path or folder_type}",
                        )
                        await websocket.send_json(response)
                    continue
            await manager.process_message(data, websocket)
    except WebSocketDisconnect:
        logger.warning("Client %s disconnected", client_id)
        manager.disconnect(websocket)


def cleanup_processes():
    logger.debug("Starting process cleanup")
    current_process = psutil.Process()
    children = []

    try:
        children = current_process.children(recursive=True)
    except psutil.Error as e:
        logger.error("Error getting child processes: %s", str(e))
        return

    # First attempt to terminate processes gracefully
    for child in children:
        try:
            if child.is_running():
                logger.debug("Attempting to terminate process: %s", child.pid)
                child.terminate()
        except (psutil.NoSuchProcess, psutil.AccessDenied, ProcessLookupError) as e:
            logger.debug(
                "Process already terminated or inaccessible: %s (%s)", child.pid, str(e)
            )
            continue
        except Exception as e:
            logger.error(
                "Unexpected error terminating process %s: %s", child.pid, str(e)
            )
            continue

    # Wait for processes to terminate and collect remaining ones
    _, alive = [], []
    try:
        _, alive = psutil.wait_procs(children, timeout=5)
    except Exception as e:
        logger.error("Error waiting for processes to terminate: %s", str(e))

    # Force kill any remaining processes
    for p in alive:
        try:
            if p.is_running():
                logger.warning("Force killing process: %s", p.pid)
                p.kill()
        except (psutil.NoSuchProcess, psutil.AccessDenied, ProcessLookupError) as e:
            logger.debug(
                "Process already terminated or inaccessible during force kill: %s (%s)",
                p.pid,
                str(e),
            )
            continue
        except Exception as e:
            logger.error("Unexpected error killing process %s: %s", p.pid, str(e))
            continue

    logger.debug("Process cleanup completed")


def start_server(host, port, dev_mode):
    logger.info("Starting server on %s:%s (dev mode: %s)", host, port, dev_mode)
    app_state.server_instance = ServerThread(app, host, port, dev_mode)
    app_state.server_instance.start()


def start_webview(url):
    logger.info("Starting webview with URL: %s", url)
    webview.settings["ALLOW_DOWNLOADS"] = True
    # Set true on Mac, False on other platforms
    webview.settings["OPEN_DEVTOOLS_IN_DEBUG"] = platform.system() == "Darwin"
    app_state.webview_window = webview.create_window(
        f"Palworld Save Pal v{__version__}",
        url,
        width=1366,
        height=768,
        min_size=(1366, 768),
    )
    app_state.webview_window.events.closed += on_closed
    webview.start(debug=True, user_agent="pywebview", private_mode=False)


def on_closed():
    logger.info("Webview window closed. Initiating shutdown...")
    app_state.terminate_flag.set()
    if app_state.server_instance:
        app_state.server_instance.stop()
    cleanup_processes()
    remove_lock_file()
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
    parser.add_argument("--web-host", type=str, help="Host to run the webview on")
    parser.add_argument("--web-port", type=int, help="Port to run the webview on")
    return parser.parse_args()


def set_mac_working_directory():
    """Set the working directory to the executable's directory on macOS."""
    if getattr(sys, "frozen", False) and platform.system() == "Darwin":
        os.chdir(os.path.dirname(sys.executable))


def get_lock_file_path():
    """Get the path for the lock file in the system temp directory."""
    import tempfile

    return Path(tempfile.gettempdir()) / "palworld_save_pal.lock"


def is_instance_running():
    """Check if another instance is already running."""
    lock_file = get_lock_file_path()

    # Check if lock file exists
    if not lock_file.exists():
        return False

    try:
        # Read PID from lock file
        with open(lock_file, "r") as f:
            pid = int(f.read().strip())

        # Check if process with this PID is still running
        if psutil.pid_exists(pid):
            try:
                process = psutil.Process(pid)
                # Additional check: verify it's our application by checking the command line
                cmdline = " ".join(process.cmdline())
                if "desktop.py" in cmdline or "palworld-save-pal" in cmdline.lower():
                    return True
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass

        # If we get here, the PID doesn't exist or isn't our app
        # Remove stale lock file
        lock_file.unlink(missing_ok=True)
        return False
    except (ValueError, FileNotFoundError, PermissionError):
        # Invalid or inaccessible lock file, consider it stale
        lock_file.unlink(missing_ok=True)
        return False


def create_lock_file():
    """Create a lock file with the current process PID."""
    lock_file = get_lock_file_path()
    try:
        with open(lock_file, "w") as f:
            f.write(str(os.getpid()))
        logger.debug("Created lock file: %s with PID: %s", lock_file, os.getpid())
    except Exception as e:
        logger.warning("Failed to create lock file: %s", str(e))


def remove_lock_file():
    """Remove the lock file."""
    lock_file = get_lock_file_path()
    try:
        lock_file.unlink(missing_ok=True)
        logger.debug("Removed lock file: %s", lock_file)
    except Exception as e:
        logger.warning("Failed to remove lock file: %s", str(e))


def bring_existing_instance_to_front():
    """Attempt to bring the existing instance to the front."""
    try:
        # Try to find the existing Palworld Save Pal window and bring it to front
        for proc in psutil.process_iter(["pid", "name", "cmdline"]):
            try:
                cmdline = " ".join(proc.info["cmdline"] or [])
                if (
                    "desktop.py" in cmdline or "palworld-save-pal" in cmdline.lower()
                ) and proc.info["pid"] != os.getpid():
                    logger.info(
                        "Found existing instance with PID: %s", proc.info["pid"]
                    )

                    # On Windows, try to bring window to foreground
                    if platform.system() == "Windows":
                        try:
                            import win32gui
                            import win32con

                            def enum_windows_callback(hwnd, results):
                                if win32gui.IsWindowVisible(hwnd):
                                    window_text = win32gui.GetWindowText(hwnd)
                                    if "Palworld Save Pal" in window_text:
                                        results.append(hwnd)

                            windows = []
                            win32gui.EnumWindows(enum_windows_callback, windows)

                            for hwnd in windows:
                                win32gui.ShowWindow(hwnd, win32con.SW_RESTORE)
                                win32gui.SetForegroundWindow(hwnd)
                                logger.info("Brought existing window to foreground")
                                break

                        except ImportError:
                            logger.debug(
                                "win32gui not available, cannot bring window to front"
                            )

                    return True
            except (psutil.NoSuchProcess, psutil.AccessDenied, IndexError):
                continue
    except Exception as e:
        logger.warning("Error trying to bring existing instance to front: %s", str(e))

    return False


def signal_handler(signum, frame):
    """Handle termination signals to ensure proper cleanup."""
    logger.info("Received signal %s, initiating graceful shutdown...", signum)
    app_state.terminate_flag.set()
    if app_state.server_instance:
        app_state.server_instance.stop()
    cleanup_processes()
    remove_lock_file()
    logger.info("Signal handler cleanup completed")
    sys.exit(0)


def main():
    set_mac_working_directory()

    multiprocessing.freeze_support()
    args = parse_arguments()
    create_db_and_tables()
    setup_logging(dev_mode=args.dev)
    global logger
    logger = create_logger(__name__)

    logger.info(
        "Starting application in %s mode on %s:%s",
        "development" if args.dev else "production",
        args.host,
        args.port,
    )

    # Check for single instance
    if is_instance_running():
        print("Another instance of Palworld Save Pal is already running.")

        # Try to bring the existing instance to the front
        if bring_existing_instance_to_front():
            print("Brought existing instance to the foreground.")
        else:
            print("Could not bring existing instance to the foreground.")

        # Exit gracefully
        sys.exit(0)

    # Create lock file to indicate this instance is running
    create_lock_file()

    # Register signal handlers for graceful shutdown
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    # Register signal handlers for graceful shutdown
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    try:
        start_server(args.host, args.port, args.dev)

        time.sleep(2)
        host = args.web_host or args.host
        port = args.web_port or args.port
        url = f"http://{host}:{port}"
        start_webview(url)

        logger.debug("Main thread waiting for termination signal")
        app_state.terminate_flag.wait()

        logger.debug("Termination signal received, initiating shutdown")
        if app_state.server_instance:
            app_state.server_instance.stop()

    finally:
        remove_lock_file()
        logger.info("Application shutdown complete, goodbye!")
        sys.exit(0)


if __name__ == "__main__":
    main()
