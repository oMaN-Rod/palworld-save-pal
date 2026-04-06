import os
import shutil
import subprocess
import zipfile
from io import BytesIO
from pathlib import Path
from typing import Any, Dict, List, Optional

import httpx
import psutil

from palworld_save_pal.db.models.server_models import ServerModel
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

STEAMCMD_ZIP_URL = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip"
PALWORLD_APP_ID = "2394010"

# Mapping from Docker-style ENV var keys to PalWorldSettings.ini OptionSettings parameter names.
# Only keys that differ between the ENV convention and the INI convention are listed.
# Keys not in this map are converted from UPPER_SNAKE_CASE to PascalCase automatically.
ENV_TO_INI: Dict[str, str] = {
    # Explicit server fields (set via dedicated UI inputs, not env accordion)
    "SERVER_NAME": "ServerName",
    "SERVER_DESCRIPTION": "ServerDescription",
    "SERVER_PASSWORD": "ServerPassword",
    "ADMIN_PASSWORD": "AdminPassword",
    "PLAYERS": "ServerPlayerMaxNum",
    "PORT": "PublicPort",
    # Gameplay rates
    "EXP_RATE": "ExpRate",
    "PAL_CAPTURE_RATE": "PalCaptureRate",
    "PAL_SPAWN_NUM_RATE": "PalSpawnNumRate",
    "PAL_DAMAGE_RATE_ATTACK": "PalDamageRateAttack",
    "PAL_DAMAGE_RATE_DEFENSE": "PalDamageRateDefense",
    "PLAYER_DAMAGE_RATE_ATTACK": "PlayerDamageRateAttack",
    "PLAYER_DAMAGE_RATE_DEFENSE": "PlayerDamageRateDefense",
    "PAL_STOMACH_DECREASE_RATE": "PalStomachDecreaceRate",
    "PAL_STAMINA_DECREASE_RATE": "PalStaminaDecreaceRate",
    "PAL_AUTO_HP_REGEN_RATE": "PalAutoHPRegeneRate",
    "PAL_AUTO_HP_REGEN_RATE_IN_SLEEP": "PalAutoHpRegeneRateInSleep",
    "PLAYER_STOMACH_DECREASE_RATE": "PlayerStomachDecreaceRate",
    "PLAYER_STAMINA_DECREASE_RATE": "PlayerStaminaDecreaceRate",
    "PLAYER_AUTO_HP_REGEN_RATE": "PlayerAutoHPRegeneRate",
    "PLAYER_AUTO_HP_REGEN_RATE_IN_SLEEP": "PlayerAutoHpRegeneRateInSleep",
    "COLLECTION_DROP_RATE": "CollectionDropRate",
    "COLLECTION_OBJECT_HP_RATE": "CollectionObjectHpRate",
    "COLLECTION_OBJECT_RESPAWN_SPEED_RATE": "CollectionObjectRespawnSpeedRate",
    "ENEMY_DROP_ITEM_RATE": "EnemyDropItemRate",
    "WORK_SPEED_RATE": "WorkSpeedRate",
    "ITEM_WEIGHT_RATE": "ItemWeightRate",
    "EQUIPMENT_DURABILITY_DAMAGE_RATE": "EquipmentDurabilityDamageRate",
    "ITEM_CORRUPTION_MULTIPLIER": "ItemCorruptionMultiplier",
    # Time & difficulty
    "DIFFICULTY": "Difficulty",
    "DAYTIME_SPEEDRATE": "DayTimeSpeedRate",
    "NIGHTTIME_SPEEDRATE": "NightTimeSpeedRate",
    "PAL_EGG_DEFAULT_HATCHING_TIME": "PalEggDefaultHatchingTime",
    "AUTO_SAVE_SPAN": "AutoSaveSpan",
    "DROP_ITEM_ALIVE_MAX_HOURS": "DropItemAliveMaxHours",
    "SUPPLY_DROP_SPAN": "SupplyDropSpan",
    # Server settings
    "PUBLIC_IP": "PublicIP",
    "PUBLIC_PORT": "PublicPort",
    "REGION": "Region",
    "USEAUTH": "bUseAuth",
    "SHOW_PLAYER_LIST": "bShowPlayerList",
    "SHOW_JOIN_LEFT_MESSAGE": "bIsShowJoinLeftMessage",
    "ALLOW_CLIENT_MOD": "bAllowClientMod",
    "CHAT_POST_LIMIT_PER_MINUTE": "ChatPostLimitPerMinute",
    "BAN_LIST_URL": "BanListURL",
    "CROSSPLAY_PLATFORMS": "CrossplayPlatforms",
    # Stat enhancement
    "ALLOW_ENHANCE_STAT_HEALTH": "bAllowEnhanceStat_Health",
    "ALLOW_ENHANCE_STAT_ATTACK": "bAllowEnhanceStat_Attack",
    "ALLOW_ENHANCE_STAT_STAMINA": "bAllowEnhanceStat_Stamina",
    "ALLOW_ENHANCE_STAT_WEIGHT": "bAllowEnhanceStat_Weight",
    "ALLOW_ENHANCE_STAT_WORK_SPEED": "bAllowEnhanceStat_WorkSpeed",
    # PvP / Hardcore
    "IS_PVP": "bIsPvP",
    "ENABLE_PLAYER_TO_PLAYER_DAMAGE": "bEnablePlayerToPlayerDamage",
    "ENABLE_FRIENDLY_FIRE": "bEnableFriendlyFire",
    "ENABLE_DEFENSE_OTHER_GUILD_PLAYER": "bEnableDefenseOtherGuildPlayer",
    "HARDCORE": "bHardcore",
    "CHARACTER_RECREATE_IN_HARDCORE": "bCharacterRecreateInHardcore",
    "PAL_LOST": "bPalLost",
    "DEATH_PENALTY": "DeathPenalty",
    "CAN_PICKUP_OTHER_GUILD_DEATH_PENALTY_DROP": "bCanPickupOtherGuildDeathPenaltyDrop",
    "ENABLE_AIM_ASSIST_PAD": "bEnableAimAssistPad",
    "ENABLE_AIM_ASSIST_KEYBOARD": "bEnableAimAssistKeyboard",
    "ENABLE_INVADER_ENEMY": "bEnableInvaderEnemy",
    "ENABLE_PREDATOR_BOSS_PAL": "EnablePredatorBossPal",
    "ENABLE_NON_LOGIN_PENALTY": "bEnableNonLoginPenalty",
    "ENABLE_FAST_TRAVEL": "bEnableFastTravel",
    "ENABLE_FAST_TRAVEL_ONLY_BASE_CAMP": "bEnableFastTravelOnlyBaseCamp",
    "EXIST_PLAYER_AFTER_LOGOUT": "bExistPlayerAfterLogout",
    "IS_START_LOCATION_SELECT_BY_MAP": "bIsStartLocationSelectByMap",
    # PvP Respawn & Rewards
    "BLOCK_RESPAWN_TIME": "BlockRespawnTime",
    "RESPAWN_PENALTY_DURATION_THRESHOLD": "RespawnPenaltyDurationThreshold",
    "RESPAWN_PENALTY_TIME_SCALE": "RespawnPenaltyTimeScale",
    "ADDITIONAL_DROP_ITEM_WHEN_PLAYER_KILLING_IN_PVP": "bAdditionalDropItemWhenPlayerKillingInPvPMode",
    "ADDITIONAL_DROP_ITEM_PVP_ITEM": "AdditionalDropItemWhenPlayerKillingInPvPMode",
    "ADDITIONAL_DROP_ITEM_PVP_NUM": "AdditionalDropItemNumWhenPlayerKillingInPvPMode",
    "DISPLAY_PVP_ITEM_NUM_ON_WORLDMAP_BASECAMP": "bDisplayPvPItemNumOnWorldMap_BaseCamp",
    "DISPLAY_PVP_ITEM_NUM_ON_WORLDMAP_PLAYER": "bDisplayPvPItemNumOnWorldMap_Player",
    # Guild / Building
    "GUILD_PLAYER_MAX_NUM": "GuildPlayerMaxNum",
    "GUILD_REJOIN_COOLDOWN_MINUTES": "GuildRejoinCooldownMinutes",
    "BASE_CAMP_MAX_NUM": "BaseCampMaxNum",
    "BASE_CAMP_MAX_NUM_IN_GUILD": "BaseCampMaxNumInGuild",
    "BASE_CAMP_WORKER_MAX_NUM": "BaseCampWorkerMaxNum",
    "BUILD_OBJECT_HP_RATE": "BuildObjectHpRate",
    "BUILD_OBJECT_DAMAGE_RATE": "BuildObjectDamageRate",
    "BUILD_OBJECT_DETERIORATION_DAMAGE_RATE": "BuildObjectDeteriorationDamageRate",
    "BUILD_AREA_LIMIT": "bBuildAreaLimit",
    "MAX_BUILDING_LIMIT_NUM": "MaxBuildingLimitNum",
    "AUTO_RESET_GUILD_NO_ONLINE_PLAYERS": "bAutoResetGuildNoOnlinePlayers",
    "AUTO_RESET_GUILD_TIME_NO_ONLINE_PLAYERS": "AutoResetGuildTimeNoOnlinePlayers",
    "INVISIBLE_OTHER_GUILD_BASE_CAMP_AREA_FX": "bInvisibleOtherGuildBaseCampAreaFX",
    # Items & Drops
    "DROP_ITEM_MAX_NUM": "DropItemMaxNum",
    "DROP_ITEM_MAX_NUM_UNKO": "DropItemMaxNum_UNKO",
    "ACTIVE_UNKO": "bActiveUNKO",
    "COOP_PLAYER_MAX_NUM": "CoopPlayerMaxNum",
    "ALLOW_GLOBAL_PALBOX_EXPORT": "bAllowGlobalPalboxExport",
    "ALLOW_GLOBAL_PALBOX_IMPORT": "bAllowGlobalPalboxImport",
    "IS_MULTIPLAY": "bIsMultiplay",
    # REST API & Logging
    "REST_API_ENABLED": "RESTAPIEnabled",
    "REST_API_PORT": "RESTAPIPort",
    "RCON_ENABLED": "RCONEnabled",
    "RCON_PORT": "RCONPort",
    "LOG_FORMAT_TYPE": "LogFormatType",
    # Backup
    "USE_BACKUP_SAVE_DATA": "bIsUseBackupSaveData",
    # Engine / Performance
    "SERVER_REPLICATE_PAWN_CULL_DISTANCE": "ServerReplicatePawnCullDistance",
    "ITEM_CONTAINER_FORCE_MARK_DIRTY_INTERVAL": "ItemContainerForceMarkDirtyInterval",
    # Randomizer
    "RANDOMIZER_TYPE": "RandomizerType",
    "RANDOMIZER_SEED": "RandomizerSeed",
    "IS_RANDOMIZER_PAL_LEVEL_RANDOM": "bIsRandomizerPalLevelRandom",
}

# Keys that are Docker-image-only (not INI parameters) — skip when writing config
DOCKER_ONLY_KEYS = {
    "MULTITHREADING", "COMMUNITY", "UPDATE_ON_BOOT",
    "ENABLE_PLAYER_LOGGING", "PLAYER_LOGGING_POLL_PERIOD",
    "LOG_FILTER_ENABLED",
    "BACKUP_ENABLED", "BACKUP_CRON_EXPRESSION", "DELETE_OLD_BACKUPS", "OLD_BACKUP_DAYS",
    "AUTO_UPDATE_ENABLED", "AUTO_UPDATE_CRON_EXPRESSION", "AUTO_UPDATE_WARN_MINUTES",
    "AUTO_REBOOT_ENABLED", "AUTO_REBOOT_CRON_EXPRESSION", "AUTO_REBOOT_WARN_MINUTES",
    "AUTO_REBOOT_EVEN_IF_PLAYERS_ONLINE",
    "USE_DEPOT_DOWNLOADER", "INSTALL_BETA_INSIDER",
    "DISCORD_WEBHOOK_URL", "DISCORD_SUPPRESS_NOTIFICATIONS",
    "DISCORD_CONNECT_TIMEOUT", "DISCORD_MAX_TIMEOUT",
    "ENABLE_UE4SS", "UE4SS_VERSION", "UE4SS_FORCE_UPDATE",
    "LAN_SERVER_MAX_TICK_RATE", "NET_SERVER_MAX_TICK_RATE",
    "SMOOTH_FRAME_RATE", "SMOOTH_FRAME_RATE_UPPER_LIMIT", "SMOOTH_FRAME_RATE_LOWER_LIMIT",
}


class NativeServerService:
    # Default install location when auto-downloading SteamCMD
    _STEAMCMD_DEFAULT_DIR = os.path.join(os.path.expanduser("~"), "SteamCMD")

    # --- SteamCMD ---

    @staticmethod
    def find_steamcmd() -> Optional[str]:
        """Auto-detect steamcmd.exe on the system. Returns path to exe or None."""
        # Check PATH
        path_dirs = os.environ.get("PATH", "").split(os.pathsep)
        for path_dir in path_dirs:
            candidate = os.path.join(path_dir, "steamcmd.exe")
            if os.path.exists(candidate):
                logger.info("Found SteamCMD in PATH at %s", candidate)
                return candidate

        # Check all drive roots for SteamCMD
        for drive_letter in "CDEFGHIJKLMNOPQRSTUVWXYZ":
            for name in ("SteamCMD", "steamcmd"):
                candidate = os.path.join(f"{drive_letter}:\\", name, "steamcmd.exe")
                if os.path.exists(candidate):
                    logger.info("Found SteamCMD at %s", candidate)
                    return candidate

        return None

    @staticmethod
    async def ensure_steamcmd(steamcmd_dir: str) -> str:
        """Download and extract SteamCMD if not already present. Returns path to steamcmd.exe."""
        steamcmd_exe = os.path.join(steamcmd_dir, "steamcmd.exe")
        if os.path.exists(steamcmd_exe):
            return steamcmd_exe

        os.makedirs(steamcmd_dir, exist_ok=True)
        logger.info("Downloading SteamCMD to %s", steamcmd_dir)

        async with httpx.AsyncClient(timeout=60.0, follow_redirects=True) as client:
            resp = await client.get(STEAMCMD_ZIP_URL)
            resp.raise_for_status()

        with zipfile.ZipFile(BytesIO(resp.content)) as zf:
            zf.extractall(steamcmd_dir)

        logger.info("SteamCMD extracted to %s", steamcmd_dir)
        return steamcmd_exe

    @staticmethod
    def install_server(steamcmd_exe: str, install_dir: str) -> bool:
        """Use SteamCMD to download/update the Palworld dedicated server."""
        os.makedirs(install_dir, exist_ok=True)
        cmd = [
            steamcmd_exe,
            "+login", "anonymous",
            "+force_install_dir", install_dir,
            "+app_update", PALWORLD_APP_ID, "validate",
            "+quit",
        ]
        logger.info("Running SteamCMD: %s", " ".join(cmd))
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=1800)
        # SteamCMD often returns non-zero exit codes even on success.
        # Check for PalServer.exe existence instead of relying on return code.
        if result.returncode != 0:
            logger.warning("SteamCMD exited with code %d (this may be normal)", result.returncode)
            if result.stdout:
                logger.info("SteamCMD stdout: %s", result.stdout[-500:])

        pal_server_exe = os.path.join(install_dir, "PalServer.exe")
        if os.path.exists(pal_server_exe):
            logger.info("SteamCMD completed successfully, PalServer.exe found at %s", install_dir)
            return True

        logger.error("SteamCMD did not produce PalServer.exe at %s", install_dir)
        if result.stdout:
            logger.error("SteamCMD output: %s", result.stdout[-1000:])
        return False

    # --- Server lifecycle ---

    # Directories to exclude when copying a base server installation.
    # These contain world-specific data, logs, or SteamCMD metadata.
    _COPY_EXCLUDE_DIRS = {"Saved", "steamapps"}

    @staticmethod
    def _copy_server_base(source: str, dest: str) -> None:
        """Copy only the base server files needed to run, excluding world data."""

        def _ignore(directory: str, entries: list) -> set:
            rel = os.path.relpath(directory, source)
            ignored = set()
            for entry in entries:
                entry_path = os.path.join(directory, entry)
                # Skip world-specific directories at any depth
                if os.path.isdir(entry_path) and entry in NativeServerService._COPY_EXCLUDE_DIRS:
                    ignored.add(entry)
            return ignored

        shutil.copytree(source, dest, ignore=_ignore, dirs_exist_ok=True)

    @staticmethod
    def create_server(server: ServerModel, source_server_path: Optional[str] = None) -> bool:
        """
        Create a native server installation.
        If source_server_path is provided and exists, copy base files from it
        (excluding world data, saves, logs, and SteamCMD metadata).
        Otherwise, use SteamCMD to download a fresh install.
        """
        install_path = server.install_path
        if not install_path:
            logger.error("install_path is required for native servers")
            return False

        if os.path.exists(os.path.join(install_path, "PalServer.exe")):
            logger.info("Server already exists at %s, skipping copy/install", install_path)
        elif source_server_path and os.path.exists(os.path.join(source_server_path, "PalServer.exe")):
            logger.info("Copying base server files from %s to %s (excluding world data)", source_server_path, install_path)
            NativeServerService._copy_server_base(source_server_path, install_path)
        else:
            logger.info("No source server found, will use SteamCMD to install")
            if not server.steamcmd_path:
                logger.error("steamcmd_path is required when no source server is available")
                return False
            success = NativeServerService.install_server(server.steamcmd_path, install_path)
            if not success:
                return False

        # Ensure config directory exists
        config_dir = os.path.join(install_path, "Pal", "Saved", "Config", "WindowsServer")
        os.makedirs(config_dir, exist_ok=True)

        # Write config files
        NativeServerService.write_palworld_settings(server)

        return True

    @staticmethod
    def start_server(server: ServerModel) -> Optional[int]:
        """Launch PalServer.exe and return the process PID."""
        exe_path = os.path.join(server.install_path, "PalServer.exe")
        if not os.path.exists(exe_path):
            logger.error("PalServer.exe not found at %s", exe_path)
            return None

        # Always rewrite config before starting to ensure INI matches DB
        NativeServerService.write_palworld_settings(server)

        args = [exe_path]
        args.append(f"-port={server.game_port}")
        args.append("-useperfthreads")
        args.append("-NoAsyncLoadingThread")
        args.append("-UseMultithreadForDS")

        # Add custom launch args
        if server.launch_args:
            for arg in server.launch_args.split():
                args.append(arg)

        logger.info("Starting PalServer: %s", " ".join(args))
        CREATE_NEW_PROCESS_GROUP = 0x00000200
        proc = subprocess.Popen(
            args,
            cwd=server.install_path,
            creationflags=CREATE_NEW_PROCESS_GROUP,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        logger.info("PalServer started with PID %d", proc.pid)
        return proc.pid

    @staticmethod
    async def stop_server(server: ServerModel, timeout: int = 30) -> bool:
        """Stop a native server. Try REST API shutdown first, then force kill."""
        pid = server.pid
        if not pid:
            logger.warning("No PID stored for server %s", server.name)
            return False

        # Try graceful shutdown via REST API
        try:
            from palworld_save_pal.services.docker_service import DockerService
            await DockerService.rest_api_call(
                "127.0.0.1", server.rest_api_port, server.admin_password,
                "shutdown", "POST", {"waittime": 5, "message": "Server shutting down..."},
            )
            logger.info("Sent shutdown command to server PID %d", pid)

            # Wait for process to exit
            try:
                proc = psutil.Process(pid)
                proc.wait(timeout=timeout)
                logger.info("Server PID %d exited gracefully", pid)
                return True
            except psutil.NoSuchProcess:
                return True
            except psutil.TimeoutExpired:
                logger.warning("Server PID %d did not exit in time, force killing", pid)
        except Exception as e:
            logger.warning("REST API shutdown failed for PID %d: %s", pid, e)

        # Force kill
        try:
            proc = psutil.Process(pid)
            proc.kill()
            proc.wait(timeout=10)
            logger.info("Force killed server PID %d", pid)
            return True
        except psutil.NoSuchProcess:
            return True
        except Exception as e:
            logger.error("Failed to kill server PID %d: %s", pid, e)
            return False

    @staticmethod
    def _get_process_tree(pid: int) -> List[psutil.Process]:
        """Get the process and all its children (PalServer.exe spawns child processes)."""
        try:
            parent = psutil.Process(pid)
            children = parent.children(recursive=True)
            return [parent] + children
        except psutil.NoSuchProcess:
            return []

    @staticmethod
    def get_process_status(pid: Optional[int]) -> Dict[str, Any]:
        """Get status of a native server process, matching Docker status format."""
        if not pid:
            return {
                "status": "exited",
                "running": False,
                "started_at": None,
                "health": None,
            }

        try:
            proc = psutil.Process(pid)
            if proc.is_running() and proc.status() != psutil.STATUS_ZOMBIE:
                import datetime
                started_at = datetime.datetime.fromtimestamp(proc.create_time()).isoformat()
                return {
                    "status": "running",
                    "running": True,
                    "started_at": started_at,
                    "health": None,
                }
        except psutil.NoSuchProcess:
            pass

        return {
            "status": "exited",
            "running": False,
            "started_at": None,
            "health": None,
        }

    @staticmethod
    def get_process_stats(pid: Optional[int]) -> Optional[Dict[str, Any]]:
        """Get process stats matching Docker stats format.

        Aggregates stats across the process tree (parent + children) since
        PalServer.exe on Windows spawns child processes that do the real work.
        """
        if not pid:
            return None

        try:
            procs = NativeServerService._get_process_tree(pid)
            if not procs:
                return None

            # Aggregate stats across all processes in the tree
            total_cpu = 0.0
            total_mem = 0
            total_disk_read = 0
            total_disk_write = 0

            for proc in procs:
                try:
                    # cpu_percent(interval=None) returns value since last call,
                    # which is non-blocking. First call returns 0.0 which is fine
                    # since we poll every 5 seconds.
                    total_cpu += proc.cpu_percent(interval=None)
                    total_mem += proc.memory_info().rss
                    try:
                        io = proc.io_counters()
                        total_disk_read += io.read_bytes
                        total_disk_write += io.write_bytes
                    except (psutil.AccessDenied, AttributeError):
                        pass
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    continue

            total_system_mem = psutil.virtual_memory().total
            mem_percent = (total_mem / total_system_mem) * 100.0 if total_system_mem > 0 else 0.0

            return {
                "cpu_percent": round(total_cpu, 2),
                "mem_usage_mb": round(total_mem / (1024 * 1024), 1),
                "mem_limit_mb": round(total_system_mem / (1024 * 1024), 1),
                "mem_percent": round(mem_percent, 1),
                "net_rx_mb": 0,
                "net_tx_mb": 0,
                "disk_read_mb": round(total_disk_read / (1024 * 1024), 2),
                "disk_write_mb": round(total_disk_write / (1024 * 1024), 2),
            }
        except psutil.NoSuchProcess:
            return None
        except Exception as e:
            logger.error("Failed to get process stats for PID %d: %s", pid, e)
            return None

    @staticmethod
    def remove_server(install_path: str, remove_data: bool = False) -> bool:
        """Remove a native server installation."""
        if remove_data and install_path and os.path.isdir(install_path):
            try:
                shutil.rmtree(install_path)
                logger.info("Removed server directory: %s", install_path)
            except Exception as e:
                logger.error("Failed to remove server directory %s: %s", install_path, e)
                return False
        return True

    # --- Config file management ---

    @staticmethod
    def write_palworld_settings(server: ServerModel) -> None:
        """Generate PalWorldSettings.ini from server model fields and env_vars."""
        config_dir = os.path.join(server.install_path, "Pal", "Saved", "Config", "WindowsServer")
        os.makedirs(config_dir, exist_ok=True)
        ini_path = os.path.join(config_dir, "PalWorldSettings.ini")

        # Start from DefaultPalWorldSettings.ini to get the full default OptionSettings
        default_ini_path = os.path.join(server.install_path, "DefaultPalWorldSettings.ini")
        defaults = NativeServerService._parse_default_settings(default_ini_path)

        # Apply env_vars first (lower priority)
        if server.env_vars:
            for env_key, env_value in server.env_vars.items():
                if env_key in DOCKER_ONLY_KEYS:
                    continue
                ini_key = ENV_TO_INI.get(env_key)
                if not ini_key:
                    continue

                # Format the value
                str_val = str(env_value)
                if not str_val:
                    continue
                defaults[ini_key] = NativeServerService._format_ini_value(ini_key, str_val)

        # Apply explicit server fields (always take precedence over env_vars)
        defaults["ServerName"] = f'"{server.server_name}"'
        defaults["ServerDescription"] = f'"{server.server_description}"'
        defaults["AdminPassword"] = f'"{server.admin_password}"'
        defaults["ServerPlayerMaxNum"] = str(server.max_players)
        defaults["PublicPort"] = str(server.game_port)
        defaults["RESTAPIEnabled"] = "True"
        defaults["RESTAPIPort"] = str(server.rest_api_port)
        if server.server_password:
            defaults["ServerPassword"] = f'"{server.server_password}"'
        else:
            defaults["ServerPassword"] = '""'

        # Build the OptionSettings line
        pairs = ",".join(f"{k}={v}" for k, v in defaults.items())
        content = (
            "[/Script/Pal.PalGameWorldSettings]\n"
            f"OptionSettings=({pairs})\n"
        )

        with open(ini_path, "w", encoding="utf-8") as f:
            f.write(content)

        logger.info("Wrote PalWorldSettings.ini to %s", ini_path)

    @staticmethod
    def _parse_default_settings(default_ini_path: str) -> Dict[str, str]:
        """Parse the DefaultPalWorldSettings.ini to get default key=value pairs."""
        defaults: Dict[str, str] = {}

        if not os.path.exists(default_ini_path):
            logger.warning("DefaultPalWorldSettings.ini not found at %s, using hardcoded defaults", default_ini_path)
            return NativeServerService._hardcoded_defaults()

        with open(default_ini_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Find OptionSettings=(...) line
        idx = content.find("OptionSettings=(")
        if idx == -1:
            return NativeServerService._hardcoded_defaults()

        start = idx + len("OptionSettings=(")
        end = content.find(")", start)
        if end == -1:
            return NativeServerService._hardcoded_defaults()

        options_str = content[start:end]

        # Parse comma-separated key=value pairs, respecting quoted strings
        pairs = NativeServerService._split_option_settings(options_str)
        for pair in pairs:
            eq_idx = pair.find("=")
            if eq_idx > 0:
                key = pair[:eq_idx].strip()
                value = pair[eq_idx + 1:].strip()
                defaults[key] = value

        return defaults

    @staticmethod
    def _split_option_settings(s: str) -> List[str]:
        """Split OptionSettings string by commas, respecting quoted strings and parentheses."""
        pairs = []
        current = []
        depth = 0
        in_quote = False

        for ch in s:
            if ch == '"' and depth == 0:
                in_quote = not in_quote
                current.append(ch)
            elif ch == '(' and not in_quote:
                depth += 1
                current.append(ch)
            elif ch == ')' and not in_quote:
                depth -= 1
                current.append(ch)
            elif ch == ',' and not in_quote and depth == 0:
                pairs.append("".join(current).strip())
                current = []
            else:
                current.append(ch)

        if current:
            pairs.append("".join(current).strip())

        return pairs

    # INI keys that are boolean fields (use True/False, not numeric)
    _BOOL_INI_KEYS = {
        "bUseAuth", "bShowPlayerList", "bIsShowJoinLeftMessage", "bAllowClientMod",
        "bAllowEnhanceStat_Health", "bAllowEnhanceStat_Attack", "bAllowEnhanceStat_Stamina",
        "bAllowEnhanceStat_Weight", "bAllowEnhanceStat_WorkSpeed",
        "bIsPvP", "bEnablePlayerToPlayerDamage", "bEnableFriendlyFire",
        "bEnableDefenseOtherGuildPlayer", "bHardcore", "bCharacterRecreateInHardcore",
        "bPalLost", "bCanPickupOtherGuildDeathPenaltyDrop",
        "bEnableAimAssistPad", "bEnableAimAssistKeyboard",
        "bEnableInvaderEnemy", "EnablePredatorBossPal",
        "bEnableNonLoginPenalty", "bEnableFastTravel", "bEnableFastTravelOnlyBaseCamp",
        "bExistPlayerAfterLogout", "bIsStartLocationSelectByMap",
        "bBuildAreaLimit", "bAutoResetGuildNoOnlinePlayers",
        "bInvisibleOtherGuildBaseCampAreaFX",
        "bActiveUNKO", "bAllowGlobalPalboxExport", "bAllowGlobalPalboxImport",
        "bIsMultiplay", "RESTAPIEnabled", "RCONEnabled",
        "bIsUseBackupSaveData", "bIsRandomizerPalLevelRandom",
        "bAdditionalDropItemWhenPlayerKillingInPvPMode",
        "bDisplayPvPItemNumOnWorldMap_BaseCamp", "bDisplayPvPItemNumOnWorldMap_Player",
    }

    @staticmethod
    def _format_ini_value(ini_key: str, value: str) -> str:
        """Format a value for INI output. Strings get quoted, bools use True/False, numbers stay raw."""
        # Keys that should be quoted strings
        string_keys = {
            "ServerName", "ServerDescription", "ServerPassword", "AdminPassword",
            "PublicIP", "Region", "BanListURL", "DeathPenalty", "Difficulty",
            "LogFormatType", "RandomizerSeed",
            "AdditionalDropItemWhenPlayerKillingInPvPMode",
        }
        if ini_key in string_keys:
            if value.startswith('"') and value.endswith('"'):
                return value
            return f'"{value}"'

        # CrossplayPlatforms uses parenthesized tuple format
        if ini_key == "CrossplayPlatforms":
            return value

        # DenyTechnologyList also special
        if ini_key == "DenyTechnologyList":
            return value

        # Only convert to bool for known boolean INI keys
        if ini_key in NativeServerService._BOOL_INI_KEYS:
            lower = value.lower()
            if lower in ("true", "1"):
                return "True"
            if lower in ("false", "0"):
                return "False"

        return value

    @staticmethod
    def _hardcoded_defaults() -> Dict[str, str]:
        """Fallback defaults matching the game's DefaultPalWorldSettings.ini."""
        return {
            "Difficulty": "None",
            "RandomizerType": "None",
            "RandomizerSeed": '""',
            "bIsRandomizerPalLevelRandom": "False",
            "DayTimeSpeedRate": "1.000000",
            "NightTimeSpeedRate": "1.000000",
            "ExpRate": "1.000000",
            "PalCaptureRate": "1.000000",
            "PalSpawnNumRate": "1.000000",
            "PalDamageRateAttack": "1.000000",
            "PalDamageRateDefense": "1.000000",
            "PlayerDamageRateAttack": "1.000000",
            "PlayerDamageRateDefense": "1.000000",
            "PlayerStomachDecreaceRate": "1.000000",
            "PlayerStaminaDecreaceRate": "1.000000",
            "PlayerAutoHPRegeneRate": "1.000000",
            "PlayerAutoHpRegeneRateInSleep": "1.000000",
            "PalStomachDecreaceRate": "1.000000",
            "PalStaminaDecreaceRate": "1.000000",
            "PalAutoHPRegeneRate": "1.000000",
            "PalAutoHpRegeneRateInSleep": "1.000000",
            "BuildObjectHpRate": "1.000000",
            "BuildObjectDamageRate": "1.000000",
            "BuildObjectDeteriorationDamageRate": "1.000000",
            "CollectionDropRate": "1.000000",
            "CollectionObjectHpRate": "1.000000",
            "CollectionObjectRespawnSpeedRate": "1.000000",
            "EnemyDropItemRate": "1.000000",
            "DeathPenalty": "All",
            "bEnablePlayerToPlayerDamage": "False",
            "bEnableFriendlyFire": "False",
            "bEnableInvaderEnemy": "True",
            "bActiveUNKO": "False",
            "bEnableAimAssistPad": "True",
            "bEnableAimAssistKeyboard": "False",
            "DropItemMaxNum": "3000",
            "DropItemMaxNum_UNKO": "100",
            "BaseCampMaxNum": "128",
            "BaseCampWorkerMaxNum": "15",
            "DropItemAliveMaxHours": "1.000000",
            "bAutoResetGuildNoOnlinePlayers": "False",
            "AutoResetGuildTimeNoOnlinePlayers": "72.000000",
            "GuildPlayerMaxNum": "20",
            "BaseCampMaxNumInGuild": "4",
            "PalEggDefaultHatchingTime": "72.000000",
            "WorkSpeedRate": "1.000000",
            "AutoSaveSpan": "30.000000",
            "bIsMultiplay": "False",
            "bIsPvP": "False",
            "bHardcore": "False",
            "bPalLost": "False",
            "bCharacterRecreateInHardcore": "False",
            "bCanPickupOtherGuildDeathPenaltyDrop": "False",
            "bEnableNonLoginPenalty": "True",
            "bEnableFastTravel": "True",
            "bEnableFastTravelOnlyBaseCamp": "False",
            "bIsStartLocationSelectByMap": "True",
            "bExistPlayerAfterLogout": "False",
            "bEnableDefenseOtherGuildPlayer": "False",
            "bInvisibleOtherGuildBaseCampAreaFX": "False",
            "bBuildAreaLimit": "False",
            "ItemWeightRate": "1.000000",
            "CoopPlayerMaxNum": "4",
            "ServerPlayerMaxNum": "32",
            "ServerName": '"Default Palworld Server"',
            "ServerDescription": '""',
            "AdminPassword": '""',
            "ServerPassword": '""',
            "bAllowClientMod": "True",
            "PublicPort": "8211",
            "PublicIP": '""',
            "RCONEnabled": "False",
            "RCONPort": "25575",
            "Region": '""',
            "bUseAuth": "True",
            "BanListURL": '"https://b.palworldgame.com/api/banlist.txt"',
            "RESTAPIEnabled": "False",
            "RESTAPIPort": "8212",
            "bShowPlayerList": "False",
            "ChatPostLimitPerMinute": "30",
            "CrossplayPlatforms": "(Steam,Xbox,PS5,Mac)",
            "bIsUseBackupSaveData": "True",
            "LogFormatType": "Text",
            "bIsShowJoinLeftMessage": "True",
            "SupplyDropSpan": "180",
            "EnablePredatorBossPal": "True",
            "MaxBuildingLimitNum": "0",
            "ServerReplicatePawnCullDistance": "15000.000000",
            "bAllowGlobalPalboxExport": "True",
            "bAllowGlobalPalboxImport": "False",
            "EquipmentDurabilityDamageRate": "1.000000",
            "ItemContainerForceMarkDirtyInterval": "1.000000",
            "ItemCorruptionMultiplier": "1.000000",
            "DenyTechnologyList": "",
            "GuildRejoinCooldownMinutes": "0",
            "BlockRespawnTime": "5.000000",
            "RespawnPenaltyDurationThreshold": "0.000000",
            "RespawnPenaltyTimeScale": "2.000000",
            "bDisplayPvPItemNumOnWorldMap_BaseCamp": "False",
            "bDisplayPvPItemNumOnWorldMap_Player": "False",
            "AdditionalDropItemWhenPlayerKillingInPvPMode": '"PlayerDropItem"',
            "AdditionalDropItemNumWhenPlayerKillingInPvPMode": "1",
            "bAdditionalDropItemWhenPlayerKillingInPvPMode": "False",
            "bAllowEnhanceStat_Health": "True",
            "bAllowEnhanceStat_Attack": "True",
            "bAllowEnhanceStat_Stamina": "True",
            "bAllowEnhanceStat_Weight": "True",
            "bAllowEnhanceStat_WorkSpeed": "True",
        }

    # --- Helpers ---

    @staticmethod
    def get_saves_path(install_path: str) -> str:
        return os.path.join(install_path, "Pal", "Saved")

    @staticmethod
    def get_mods_path(install_path: str) -> str:
        return os.path.join(install_path, "Pal", "Binaries", "Win64", "Mods")

    @staticmethod
    def get_logicmods_path(install_path: str) -> str:
        return os.path.join(install_path, "Pal", "Content", "Paks", "LogicMods")

    @staticmethod
    def get_nativemods_path(install_path: str) -> str:
        return os.path.join(install_path, "Pal", "Binaries", "Win64", "NativeMods")

    @staticmethod
    def find_existing_server(steamcmd_path: str, install_path: str = "") -> Optional[str]:
        """Find an existing PalServer installation.

        Searches:
        1. Sibling directories of the install_path (e.g., if install_path is
           O:\\PalworldServers\\MyWorld, checks O:\\PalworldServers\\*\\PalServer.exe)
        2. The default SteamCMD app install location
        """
        # Search siblings of the install path for any existing PalServer
        if install_path:
            base_dir = os.path.dirname(install_path)
            if os.path.isdir(base_dir):
                for entry in os.listdir(base_dir):
                    candidate = os.path.join(base_dir, entry)
                    if candidate == install_path:
                        continue
                    if os.path.exists(os.path.join(candidate, "PalServer.exe")):
                        logger.info("Found existing PalServer at sibling: %s", candidate)
                        return candidate

        # Search under SteamCMD directory
        if steamcmd_path:
            steamcmd_dir = os.path.dirname(steamcmd_path) if steamcmd_path.endswith(".exe") else steamcmd_path
            default_path = os.path.join(steamcmd_dir, "steamapps", "common", "PalServer")
            if os.path.exists(os.path.join(default_path, "PalServer.exe")):
                logger.info("Found existing PalServer at SteamCMD default: %s", default_path)
                return default_path

        return None
