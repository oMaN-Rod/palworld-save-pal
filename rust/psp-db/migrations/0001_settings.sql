-- Single-row settings table (id is always 1), mirroring
-- palworld_save_pal/db/models/settings_model.py. Full schema lands in Phase 3.
CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    language TEXT NOT NULL,
    save_dir TEXT NOT NULL,
    clone_prefix TEXT NOT NULL,
    new_pal_prefix TEXT NOT NULL,
    debug_mode INTEGER NOT NULL,
    cheat_mode INTEGER NOT NULL
);
