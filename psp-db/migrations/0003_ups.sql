CREATE TABLE ups_collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    color TEXT,
    icon TEXT,
    is_favorite INTEGER NOT NULL DEFAULT 0,
    is_archived INTEGER NOT NULL DEFAULT 0,
    pal_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_ups_collections_name ON ups_collections(name);

CREATE TABLE ups_pals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_id TEXT NOT NULL UNIQUE,
    character_id TEXT NOT NULL,
    nickname TEXT,
    level INTEGER NOT NULL DEFAULT 1,
    pal_data TEXT NOT NULL,
    source_save_file TEXT,
    source_player_uid TEXT,
    source_player_name TEXT,
    source_storage_type TEXT,
    source_storage_slot INTEGER,
    collection_id INTEGER REFERENCES ups_collections(id) ON DELETE SET NULL,
    tags TEXT NOT NULL DEFAULT '[]',
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_accessed_at TEXT,
    transfer_count INTEGER NOT NULL DEFAULT 0,
    clone_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_ups_pals_character_id ON ups_pals(character_id);
CREATE INDEX idx_ups_pals_nickname ON ups_pals(nickname);
CREATE INDEX idx_ups_pals_level ON ups_pals(level);
CREATE INDEX idx_ups_pals_collection_id ON ups_pals(collection_id);
CREATE INDEX idx_ups_pals_created_at ON ups_pals(created_at);

CREATE TABLE ups_tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    color TEXT,
    usage_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE ups_stats (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    total_pals INTEGER NOT NULL DEFAULT 0,
    total_collections INTEGER NOT NULL DEFAULT 0,
    total_tags INTEGER NOT NULL DEFAULT 0,
    total_transfers INTEGER NOT NULL DEFAULT 0,
    total_clones INTEGER NOT NULL DEFAULT 0,
    storage_size_mb REAL NOT NULL DEFAULT 0.0,
    most_transferred_pal_id INTEGER,
    most_cloned_pal_id INTEGER,
    most_popular_character_id TEXT,
    element_distribution TEXT NOT NULL DEFAULT '{}',
    alpha_count INTEGER NOT NULL DEFAULT 0,
    lucky_count INTEGER NOT NULL DEFAULT 0,
    human_count INTEGER NOT NULL DEFAULT 0,
    predator_count INTEGER NOT NULL DEFAULT 0,
    oilrig_count INTEGER NOT NULL DEFAULT 0,
    summon_count INTEGER NOT NULL DEFAULT 0,
    last_updated TEXT NOT NULL
);

CREATE TABLE ups_transfer_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pal_id INTEGER NOT NULL,
    operation_type TEXT NOT NULL,
    source_type TEXT,
    source_location TEXT,
    destination_type TEXT,
    destination_location TEXT,
    save_file_name TEXT,
    player_name TEXT,
    player_uid TEXT,
    success INTEGER NOT NULL DEFAULT 1,
    error_message TEXT,
    timestamp TEXT NOT NULL
);
CREATE INDEX idx_ups_transfer_log_pal_id ON ups_transfer_log(pal_id);
CREATE INDEX idx_ups_transfer_log_operation_type ON ups_transfer_log(operation_type);
CREATE INDEX idx_ups_transfer_log_timestamp ON ups_transfer_log(timestamp);
