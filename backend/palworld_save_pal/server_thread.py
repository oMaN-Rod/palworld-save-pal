import threading

import uvicorn

from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class ServerThread(threading.Thread):
    def __init__(self, app, host, port, dev_mode):
        super().__init__()
        self.app = app
        self.host = host
        self.port = port
        self.dev_mode = dev_mode
        self.server = None

    def run(self):
        logger.debug("Starting server thread")
        config = uvicorn.Config(
            app=self.app,
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
