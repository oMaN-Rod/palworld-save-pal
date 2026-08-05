CREATE TABLE blueprints (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    source_world     TEXT NOT NULL,
    source_base      TEXT NOT NULL,
    created_at       INTEGER NOT NULL,
    schema_version   INTEGER NOT NULL,
    structure_count  INTEGER NOT NULL,
    manifest         TEXT NOT NULL,
    footprint_radius REAL NOT NULL,
    payload          BLOB NOT NULL,
    preview          BLOB
);
