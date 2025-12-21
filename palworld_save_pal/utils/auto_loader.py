"""Auto-loader for mounted save directories.

This module handles loading Palworld save files directly from a mounted directory,
bypassing the normal zip upload process. It reads raw .sav files and prepares them
for processing - NO UNZIPPING REQUIRED since the files are already in .sav format.

Flow comparison:
- Normal: User uploads ZIP → unzip → extract .sav bytes → process
- Auto-load: Read .sav files directly → process (skips unzip entirely)
"""
import os
import zipfile
from pathlib import Path
from typing import Dict, Optional
import uuid
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


def check_mounted_saves(mount_path: str = "/app/saves") -> Optional[Dict]:
    """
    Check if there's a mounted save directory and prepare it for loading.
    
    NOTE: This reads RAW .sav files directly - no zip extraction needed!
    The mounted directory should contain Level.sav, LevelMeta.sav, and Players/ folder.
    
    Args:
        mount_path: Path where saves are mounted in the container
        
    Returns:
        Dict with save data ready for processing, or None if no valid saves found
    """
    if not os.path.exists(mount_path):
        logger.info(f"No mounted save directory found at {mount_path}")
        return None
    
    logger.info(f"Found mounted save directory at {mount_path}")
    
    # Check for Level.sav (required file)
    level_sav_path = Path(mount_path) / "Level.sav"
    if not level_sav_path.exists():
        logger.warning(f"No Level.sav found in {mount_path}")
        return None
    
    # Check for LevelMeta.sav (optional but recommended)
    level_meta_path = Path(mount_path) / "LevelMeta.sav"
    level_meta_data = None
    if level_meta_path.exists():
        with open(level_meta_path, "rb") as f:
            level_meta_data = f.read()
        logger.info("Found LevelMeta.sav")
    
    # Read Level.sav
    with open(level_sav_path, "rb") as f:
        level_sav_data = f.read()
    logger.info(f"Loaded Level.sav ({len(level_sav_data)} bytes)")
    
    # Check for Players directory
    players_dir = Path(mount_path) / "Players"
    if not players_dir.exists() or not players_dir.is_dir():
        logger.warning(f"No Players directory found in {mount_path}")
        return None
    
    # Load player saves
    player_saves: Dict[uuid.UUID, Dict[str, bytes]] = {}
    for player_file in players_dir.glob("*.sav"):
        player_id = player_file.stem  # filename without extension
        
        # Handle DPS files
        dps = False
        if "_dps" in player_id:
            player_id = player_id.replace("_dps", "")
            dps = True
        
        try:
            player_uuid = uuid.UUID(player_id)
        except ValueError:
            logger.warning(f"Skipping invalid player file: {player_file.name}")
            continue
        
        if player_uuid not in player_saves:
            player_saves[player_uuid] = {}
        
        save_type = "dps" if dps else "sav"
        with open(player_file, "rb") as f:
            player_saves[player_uuid][save_type] = f.read()
        
        logger.info(f"Loaded player {player_uuid} ({save_type})")
    
    if not player_saves:
        logger.warning("No valid player save files found")
        return None
    
    # Use the mount path name as save_id, or a default
    save_id = Path(mount_path).name
    if save_id == "saves":
        save_id = "0"  # Default to server ID "0"
    
    logger.info(f"Successfully prepared auto-load data with {len(player_saves)} players")
    
    return {
        "save_id": save_id,
        "level_sav": level_sav_data,
        "level_meta": level_meta_data,
        "player_saves": player_saves,
    }


def create_watch_mode(mount_path: str = "/app/saves", callback=None):
    """
    Optional: Watch the mounted directory for changes and trigger reload.
    This is more advanced and requires file watching libraries.
    """
    # TODO: Implement file watching if needed
    pass
