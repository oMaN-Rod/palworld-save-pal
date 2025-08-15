import sqlite3
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


def check_column_exists(cursor, table, column):
    cursor.execute(f"PRAGMA table_info({table})")
    columns = [info[1] for info in cursor.fetchall()]
    return column in columns


def migrate_table_column(
    conn, cursor, table_name, column_name, column_type, default_value
):
    cursor.execute(
        f"SELECT name FROM sqlite_master WHERE type='table' AND name='{table_name}'"
    )
    if not cursor.fetchone():
        logger.debug(
            f"{table_name} table doesn't exist yet, skipping {column_name} migration"
        )
        return

    if not check_column_exists(cursor, table_name, column_name):
        logger.debug(f"Adding {column_name} column to {table_name} table")
        cursor.execute(
            f"ALTER TABLE {table_name} ADD COLUMN {column_name} {column_type} NOT NULL DEFAULT {default_value}"
        )
        conn.commit()
        logger.debug("element column added successfully")
    else:
        logger.debug(
            f"{column_name} column already exists in {table_name}, skipping migration"
        )


def run_migrations(db_path):
    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()

        migrate_table_column(conn, cursor, "settingsmodel", "cheat_mode", "BOOLEAN", 0)
        migrate_table_column(conn, cursor, "palpreset", "lock_element", "BOOLEAN", 0)
        migrate_table_column(conn, cursor, "palpreset", "element", "TEXT", "''")
        # Added in v0.15.0
        migrate_table_column(conn, cursor, "palpreset", "nickname", "TEXT", "''")
        migrate_table_column(
            conn, cursor, "palpreset", "filtered_nickname", "TEXT", "''"
        )
        migrate_table_column(conn, cursor, "palpreset", "stomach", "REAL", 0.0)
        migrate_table_column(conn, cursor, "palpreset", "hp", "INTEGER", 0)
        migrate_table_column(
            conn, cursor, "palpreset", "friendship_point", "INTEGER", 0
        )

        migrate_table_column(
            conn, cursor, "ups_stats", "element_distribution", "TEXT", "'{}'"
        )
        migrate_table_column(conn, cursor, "ups_stats", "alpha_count", "INTEGER", 0)
        migrate_table_column(conn, cursor, "ups_stats", "lucky_count", "INTEGER", 0)
        migrate_table_column(conn, cursor, "ups_stats", "human_count", "INTEGER", 0)
        migrate_table_column(conn, cursor, "ups_stats", "predator_count", "INTEGER", 0)
        migrate_table_column(conn, cursor, "ups_stats", "oilrig_count", "INTEGER", 0)
        migrate_table_column(conn, cursor, "ups_stats", "summon_count", "INTEGER", 0)

        cursor.close()
        conn.close()
        logger.debug("All migrations completed")

    except Exception as e:
        logger.error(f"Error during database migration: {str(e)}")
        # Don't raise the exception - we want the application to continue
        # even if migrations fail
