---
title: Save Management
description: Understanding save file formats, backups, and conversions
---

# Save Management

Palworld uses Unreal Engine's `.sav` format for storing game data. This guide covers how save files work and how to manage them effectively.

## Save File Structure {.toc}

A typical Palworld save directory contains:

```
SaveGames/
└── 0/
    └── <WorldID>/
        ├── Level.sav          # World data
        ├── LevelMeta.sav      # World metadata
        ├── LocalData.sav      # Local settings
        └── Players/
            └── <PlayerUID>.sav  # Per-player data
```

## Steam vs GamePass Saves {.toc}

Palworld saves differ between Steam and GamePass (Xbox) versions:

- **Steam** saves use the standard `.sav` format
- **GamePass** saves use a different container format

### Converting Between Formats

Palworld Save Pal includes conversion tools accessible from the **Tools** page:

1. Navigate to **Tools** in the navigation bar
2. Select the conversion direction (Steam → GamePass or GamePass → Steam)
3. Select your source save file
4. The converted file will be saved to your chosen location

## Backups {.toc}

### Automatic Backups

When using the desktop app, Palworld Save Pal automatically creates backups before saving changes. Backups are stored alongside `PSP.exe`.

### Manual Backups

It's recommended to maintain manual backups as well:

1. Copy the entire save directory to a safe location
2. Label backups with the date and a description of the current state
3. Store backups on a separate drive when possible
