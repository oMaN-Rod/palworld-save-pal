CREATE TABLE IF NOT EXISTS amity_instances (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  name       TEXT NOT NULL,
  host       TEXT NOT NULL,
  port       INTEGER NOT NULL,
  token      TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
