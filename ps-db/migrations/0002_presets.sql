CREATE TABLE presets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    preset_type TEXT NOT NULL,
    skills TEXT,
    common_container TEXT,
    essential_container TEXT,
    weapon_load_out_container TEXT,
    player_equipment_armor_container TEXT,
    food_equip_container TEXT,
    storage_container TEXT,
    pal_preset TEXT
);
