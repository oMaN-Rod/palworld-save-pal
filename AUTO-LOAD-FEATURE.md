# Auto-Load & Reload Feature

This feature allows Palworld Save Pal to automatically load save files from a mounted directory at startup, and provides a "Reload Save" button to refresh the save without restarting the container.

## ✨ Features Added

1. **Auto-load on startup** - Automatically reads `.sav` files from mounted directory
2. **Reload Save button** - Green button in the UI to refresh saves live (only visible when a save is auto-loaded)
3. **No unzipping needed** - Reads raw `.sav` files directly, bypassing zip extraction

## 🚀 How It Works

### Startup Flow
```
Container starts → Check /app/saves → Find .sav files → Load directly → Ready!
```

### Reload Flow
```
Click "Reload Save" button → Re-read /app/saves → Load fresh data → UI updates
```

## 📝 Configuration

### 1. Mount Your Save Directory

Edit `docker-compose.yml`:
```yaml
volumes:
  - /path/to/your/SaveGames/0/YOUR-WORLD-ID:/app/saves:ro
environment:
  - SAVE_MOUNT_PATH=/app/saves
```

**Example:**
```yaml
volumes:
  - /home/paul/.gamedata/palworld/Pal/Saved/SaveGames/0/E78D2AA4834049EF90A165AE9CBB433D:/app/saves:ro
environment:
  - SAVE_MOUNT_PATH=/app/saves
```

### 2. Directory Structure Expected

Your mounted directory should contain:
```
/app/saves/
├── Level.sav          (required)
├── LevelMeta.sav      (optional)
└── Players/           (required)
    ├── PLAYER-UUID-1.sav
    ├── PLAYER-UUID-2.sav
    └── ...
```

### 3. Start Container

```bash
docker compose up --build -d
```

Check logs to verify auto-load:
```bash
docker logs palworld-save-pal-backend-1
```

Look for:
```
✅ Auto-load successful! Save file ready.
```

## 🔄 Using the Reload Feature

1. The **"Reload Save"** button appears in the top-right (green, next to Discord button)
2. Only visible when `appState.saveFile.local === true` (auto-loaded saves)
3. Click it to reload fresh data from the mounted directory
4. Shows loading progress with messages
5. Updates all players, guilds, and pals automatically

## 🛠️ Files Modified

- `palworld_save_pal/utils/auto_loader.py` - New: Auto-load logic
- `palworld_save_pal/ws/handlers/save_file_handler.py` - Added: `reload_mounted_save_handler`
- `palworld_save_pal/ws/messages.py` - Added: `RELOAD_MOUNTED_SAVE` message type
- `palworld_save_pal/ws/handlers/bootstrap.py` - Registered reload handler
- `psp.py` - Added startup event to auto-load saves
- `docker-compose.yml` - Configured volume mount and env var
- `ui/src/lib/types/ws.ts` - Added message type
- `ui/src/routes/edit/+layout.svelte` - Added Reload Save button

## 📊 Data Storage

**Important:** The save files are parsed into memory. The `data/` directory contains:
- SQLite database (presets, settings, UPS collections)
- Static reference JSON files (items, pals, skills, etc.)
- **NOT your actual save data** - that stays in memory

## 🔍 Troubleshooting

### No auto-load on startup
```bash
# Check if mount path exists
docker exec palworld-save-pal-backend-1 ls -la /app/saves

# Check environment variable
docker exec palworld-save-pal-backend-1 env | grep SAVE_MOUNT_PATH
```

### Reload button not showing
- Only appears when save is auto-loaded (local=true)
- Check if you manually uploaded a zip instead

### Save not updating after reload
- Ensure mount is NOT read-only if Palworld is writing to it
- Or remount after Palworld updates the files

## 💡 Benefits

- **No manual upload needed** - Just start the container
- **Live updates** - Reload without restarting
- **Read-only safe** - Mount with `:ro` flag to prevent accidental writes
- **Faster workflow** - Skip zip/unzip entirely
