-- Signal (live world feed) settings. A single row, id = 1.
-- Deliberately narrow: the dedicated-server AdminPassword is NEVER stored —
-- it lives only in the running poller's memory.
CREATE TABLE signal_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 0,
    bind TEXT NOT NULL DEFAULT '127.0.0.1',
    port INTEGER NOT NULL DEFAULT 8788,
    interval_ms INTEGER NOT NULL DEFAULT 1000,
    allowed_origins TEXT NOT NULL DEFAULT '[]',
    source_type TEXT,
    source_url TEXT,
    gamedata_path TEXT,
    token TEXT NOT NULL DEFAULT ''
);
