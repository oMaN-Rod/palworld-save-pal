CREATE TABLE signal_devices (device_id TEXT PRIMARY KEY, secret_hex TEXT NOT NULL, name TEXT NOT NULL, created_at_ms INTEGER NOT NULL, last_seen_ms INTEGER);
