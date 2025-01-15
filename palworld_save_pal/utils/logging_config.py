import logging
import logging.config
from datetime import datetime
from pathlib import Path

from colorlog import ColoredFormatter


def setup_logging(dev_mode: bool = False):
    print("Setting up logging configuration with dev_mode: ", dev_mode)
    date = datetime.now().strftime("%m-%d-%y")
    log_dir = Path("logs")
    log_dir.mkdir(exist_ok=True)
    log_file = log_dir / f"app_{date}.log"

    LOG_CONFIG = {
        "version": 1,
        "disable_existing_loggers": False,
        "formatters": {
            "standard": {
                "format": "%(asctime)s [%(levelname)s] %(name)s.%(funcName)s: %(message)s"
            },
            "colored": {
                "()": ColoredFormatter,
                "format": "%(asctime)s [%(log_color)s%(levelname)s%(reset)s] %(name)s.%(cyan)s%(funcName)s%(reset)s:%(lineno)s => %(white)s%(message)s",
                "log_colors": {
                    "DEBUG": "blue",
                    "INFO": "green",
                    "WARNING": "yellow",
                    "ERROR": "red",
                    "CRITICAL": "red",
                },
            },
        },
        "handlers": {
            "console": {
                "level": "DEBUG" if dev_mode else "INFO",
                "formatter": "colored",
                "class": "logging.StreamHandler",
                "stream": "ext://sys.stdout",
            },
            "file": {
                "level": "DEBUG",
                "formatter": "standard",
                "class": "logging.FileHandler",
                "filename": str(log_file),
                "mode": "a",
                "encoding": "utf-8",
            },
        },
        "loggers": {
            "": {
                "handlers": ["console", "file"] if dev_mode else ["console"],
                "level": "DEBUG" if dev_mode else "INFO",
                "propagate": True,
            }
        },
    }

    logging.config.dictConfig(LOG_CONFIG)


def create_logger(name: str) -> logging.Logger:
    return logging.getLogger(name)
