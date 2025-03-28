import sqlite3
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


def check_column_exists(cursor, table, column):
    cursor.execute(f"PRAGMA table_info({table})")
    columns = [info[1] for info in cursor.fetchall()]
    return column in columns


def migrate_add_cheat_mode(conn, cursor):
    cursor.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='settingsmodel'"
    )
    if not cursor.fetchone():
        logger.debug(
            "settingsmodel table doesn't exist yet, skipping cheat_mode migration"
        )
        return

    if not check_column_exists(cursor, "settingsmodel", "cheat_mode"):
        logger.info("Adding cheat_mode column to settingsmodel table")
        cursor.execute(
            "ALTER TABLE settingsmodel ADD COLUMN cheat_mode BOOLEAN NOT NULL DEFAULT 0"
        )
        conn.commit()
        logger.info("cheat_mode column added successfully")
    else:
        logger.debug("cheat_mode column already exists, skipping migration")


def run_migrations(db_path):
    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()

        migrate_add_cheat_mode(conn, cursor)

        cursor.close()
        conn.close()
        logger.info("All migrations completed")

    except Exception as e:
        logger.error(f"Error during database migration: {str(e)}")
        # Don't raise the exception - we want the application to continue
        # even if migrations fail
