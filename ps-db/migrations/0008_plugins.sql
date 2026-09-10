CREATE TABLE IF NOT EXISTS plugins (
    id                   TEXT PRIMARY KEY,
    manifest             TEXT    NOT NULL,
    sources              TEXT    NOT NULL,
    enabled              INTEGER NOT NULL DEFAULT 1,
    granted_capabilities TEXT    NOT NULL DEFAULT '[]',
    bundled              INTEGER NOT NULL DEFAULT 0,
    installed_at         TEXT    NOT NULL,
    updated_at           TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS plugin_storage (
    plugin_id TEXT NOT NULL,
    key       TEXT NOT NULL,
    value     TEXT NOT NULL,
    PRIMARY KEY (plugin_id, key),
    FOREIGN KEY (plugin_id) REFERENCES plugins(id) ON DELETE CASCADE
);
