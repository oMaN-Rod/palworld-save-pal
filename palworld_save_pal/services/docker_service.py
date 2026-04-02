import base64
import os
import shutil
import zipfile
from io import BytesIO
from pathlib import Path
from typing import Any, Dict, List, Optional, Set

import docker
from docker.errors import DockerException, NotFound, APIError
import httpx

from palworld_save_pal.db.models.server_models import ServerModel
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class DockerService:
    _client: Optional[docker.DockerClient] = None

    @classmethod
    def get_client(cls) -> docker.DockerClient:
        if cls._client is None:
            try:
                cls._client = docker.from_env()
                cls._client.ping()
            except DockerException as e:
                logger.error("Failed to connect to Docker: %s", e)
                cls._client = None
                raise
        return cls._client

    # --- Container lifecycle ---

    @staticmethod
    def create_server(server: ServerModel) -> str:
        client = DockerService.get_client()

        # Create host directories
        for path in [server.saves_path, server.mods_path, server.logicmods_path, server.nativemods_path]:
            os.makedirs(path, exist_ok=True)

        env = DockerService.build_environment(server)
        ports = DockerService.build_port_bindings(server)
        volumes = DockerService.build_volumes(server)

        container = client.containers.create(
            image=server.image_name,
            name=server.container_name,
            detach=True,
            restart_policy={"Name": "unless-stopped"},
            stop_signal="SIGTERM",
            environment=env,
            ports=ports,
            volumes=volumes,
        )

        container.start()
        logger.info("Created and started container: %s", server.container_name)
        return container.id

    @staticmethod
    def start_server(container_name: str) -> bool:
        try:
            client = DockerService.get_client()
            container = client.containers.get(container_name)
            container.start()
            logger.info("Started container: %s", container_name)
            return True
        except (NotFound, APIError) as e:
            logger.error("Failed to start container %s: %s", container_name, e)
            return False

    @staticmethod
    def stop_server(container_name: str, timeout: int = 30) -> bool:
        try:
            client = DockerService.get_client()
            container = client.containers.get(container_name)
            container.stop(timeout=timeout)
            logger.info("Stopped container: %s", container_name)
            return True
        except (NotFound, APIError) as e:
            logger.error("Failed to stop container %s: %s", container_name, e)
            return False

    @staticmethod
    def remove_server(container_name: str, remove_volumes: bool = False) -> bool:
        try:
            client = DockerService.get_client()
            container = client.containers.get(container_name)
            container.remove(force=True)
            logger.info("Removed container: %s", container_name)

            if remove_volumes:
                try:
                    vol = client.volumes.get(f"psp-{container_name}-data")
                    vol.remove()
                    logger.info("Removed volume: psp-%s-data", container_name)
                except NotFound:
                    pass

            return True
        except (NotFound, APIError) as e:
            logger.error("Failed to remove container %s: %s", container_name, e)
            return False

    @staticmethod
    def get_container_status(container_name: str) -> Optional[Dict[str, Any]]:
        try:
            client = DockerService.get_client()
            container = client.containers.get(container_name)
            state = container.attrs.get("State", {})
            return {
                "status": container.status,
                "running": state.get("Running", False),
                "started_at": state.get("StartedAt"),
                "health": state.get("Health", {}).get("Status")
                if state.get("Health")
                else None,
            }
        except NotFound:
            return {
                "status": "not_found",
                "running": False,
                "started_at": None,
                "health": None,
            }
        except DockerException as e:
            logger.error("Failed to get status for %s: %s", container_name, e)
            return None

    @staticmethod
    def get_container_stats(container_name: str) -> Optional[Dict[str, Any]]:
        try:
            client = DockerService.get_client()
            container = client.containers.get(container_name)
            if container.status != "running":
                return None

            stats = container.stats(stream=False)

            # CPU usage calculation
            cpu_delta = (
                stats["cpu_stats"]["cpu_usage"]["total_usage"]
                - stats["precpu_stats"]["cpu_usage"]["total_usage"]
            )
            system_delta = (
                stats["cpu_stats"]["system_cpu_usage"]
                - stats["precpu_stats"]["system_cpu_usage"]
            )
            num_cpus = stats["cpu_stats"].get("online_cpus", 1)
            cpu_percent = (cpu_delta / system_delta) * num_cpus * 100.0 if system_delta > 0 else 0.0

            # Memory usage
            mem_stats = stats.get("memory_stats", {})
            mem_usage = mem_stats.get("usage", 0)
            mem_limit = mem_stats.get("limit", 1)
            mem_percent = (mem_usage / mem_limit) * 100.0 if mem_limit > 0 else 0.0

            # Network I/O
            net_stats = stats.get("networks", {})
            net_rx = sum(v.get("rx_bytes", 0) for v in net_stats.values())
            net_tx = sum(v.get("tx_bytes", 0) for v in net_stats.values())

            # Block I/O
            blkio = stats.get("blkio_stats", {}).get("io_service_bytes_recursive", []) or []
            disk_read = sum(e.get("value", 0) for e in blkio if e.get("op") == "read")
            disk_write = sum(e.get("value", 0) for e in blkio if e.get("op") == "write")

            return {
                "cpu_percent": round(cpu_percent, 2),
                "mem_usage_mb": round(mem_usage / (1024 * 1024), 1),
                "mem_limit_mb": round(mem_limit / (1024 * 1024), 1),
                "mem_percent": round(mem_percent, 1),
                "net_rx_mb": round(net_rx / (1024 * 1024), 2),
                "net_tx_mb": round(net_tx / (1024 * 1024), 2),
                "disk_read_mb": round(disk_read / (1024 * 1024), 2),
                "disk_write_mb": round(disk_write / (1024 * 1024), 2),
            }
        except NotFound:
            return None
        except Exception as e:
            logger.error("Failed to get stats for %s: %s", container_name, e)
            return None

    @staticmethod
    def list_containers(image_filter: str = "omanrod/psp-palworld-server") -> List[Dict[str, Any]]:
        try:
            client = DockerService.get_client()
            containers = client.containers.list(all=True)
            result = []
            for c in containers:
                image_tags = c.image.tags if c.image.tags else []
                if any(image_filter in tag for tag in image_tags):
                    result.append(
                        {
                            "container_name": c.name,
                            "status": c.status,
                            "image": image_tags[0] if image_tags else "unknown",
                        }
                    )
            return result
        except DockerException as e:
            logger.error("Failed to list containers: %s", e)
            return []

    # --- REST API proxy ---

    @staticmethod
    async def rest_api_call(
        host: str,
        port: int,
        admin_password: str,
        endpoint: str,
        method: str = "GET",
        data: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        url = f"http://{host}:{port}/v1/api/{endpoint}"
        auth = httpx.BasicAuth("admin", admin_password)

        async with httpx.AsyncClient(timeout=10.0) as client:
            if method.upper() == "GET":
                response = await client.get(url, auth=auth)
            elif method.upper() == "POST":
                response = await client.post(url, json=data or {}, auth=auth)
            else:
                response = await client.request(
                    method.upper(), url, json=data, auth=auth
                )

            return {
                "status_code": response.status_code,
                "data": response.json()
                if response.headers.get("content-type", "").startswith(
                    "application/json"
                )
                else response.text,
            }

    @staticmethod
    async def get_player_count(host: str, port: int, admin_password: str) -> int:
        try:
            result = await DockerService.rest_api_call(
                host, port, admin_password, "players"
            )
            players = result.get("data", {}).get("players", [])
            return len(players)
        except Exception as e:
            logger.error("Failed to get player count: %s", e)
            return 0

    # --- Mod management ---

    @staticmethod
    def list_mods(mods_path: str) -> List[Dict[str, Any]]:
        mods = []
        mods_txt = os.path.join(mods_path, "mods.txt")

        # Parse mods.txt for enabled/disabled state
        enabled_map: Dict[str, bool] = {}
        if os.path.exists(mods_txt):
            with open(mods_txt, "r") as f:
                for line in f:
                    line = line.strip()
                    if " : " in line:
                        parts = line.split(" : ")
                        mod_name = parts[0].strip()
                        enabled = parts[1].strip() == "1"
                        enabled_map[mod_name] = enabled

        # Scan directories for mod folders
        if os.path.isdir(mods_path):
            for entry in os.listdir(mods_path):
                entry_path = os.path.join(mods_path, entry)
                if os.path.isdir(entry_path) and entry != "shared":
                    mods.append(
                        {
                            "mod_name": entry,
                            "mod_type": "ue4ss",
                            "enabled": enabled_map.get(entry, False),
                        }
                    )

        return mods

    @staticmethod
    def set_mod_enabled(mods_path: str, mod_name: str, enabled: bool) -> bool:
        mods_txt = os.path.join(mods_path, "mods.txt")

        lines = []
        found = False
        if os.path.exists(mods_txt):
            with open(mods_txt, "r") as f:
                for line in f:
                    stripped = line.strip()
                    if stripped.startswith(mod_name + " : "):
                        lines.append(f"{mod_name} : {'1' if enabled else '0'}\n")
                        found = True
                    else:
                        lines.append(line)

        if not found:
            lines.append(f"{mod_name} : {'1' if enabled else '0'}\n")

        with open(mods_txt, "w") as f:
            f.writelines(lines)

        return True

    @staticmethod
    def install_mod(
        mods_path: str, mod_name: str, mod_zip_b64: str, mod_type: str = "ue4ss"
    ) -> bool:
        try:
            zip_bytes = base64.b64decode(mod_zip_b64)
            mod_dir = os.path.join(mods_path, mod_name)
            os.makedirs(mod_dir, exist_ok=True)

            with zipfile.ZipFile(BytesIO(zip_bytes)) as zf:
                zf.extractall(mod_dir)

            # Add to mods.txt as enabled
            DockerService.set_mod_enabled(mods_path, mod_name, True)
            logger.info("Installed mod: %s", mod_name)
            return True
        except Exception as e:
            logger.error("Failed to install mod %s: %s", mod_name, e)
            return False

    @staticmethod
    def list_native_mods(nativemods_path: str) -> List[Dict[str, Any]]:
        mods = []
        if not os.path.isdir(nativemods_path):
            return mods

        for entry in os.listdir(nativemods_path):
            entry_path = os.path.join(nativemods_path, entry)
            if entry.startswith("."):
                continue
            if os.path.isfile(entry_path) and entry.endswith(".dll"):
                mods.append({
                    "mod_name": entry,
                    "mod_type": "native",
                    "enabled": True,
                })
            elif os.path.isdir(entry_path):
                # Mod config directories (e.g., PalDefender/)
                dll_count = len([f for f in os.listdir(entry_path) if f.endswith(".dll")])
                if dll_count > 0:
                    mods.append({
                        "mod_name": entry,
                        "mod_type": "native",
                        "enabled": True,
                    })
        return mods

    @staticmethod
    def install_native_mod(nativemods_path: str, mod_name: str, mod_zip_b64: str) -> bool:
        try:
            zip_bytes = base64.b64decode(mod_zip_b64)
            os.makedirs(nativemods_path, exist_ok=True)

            with zipfile.ZipFile(BytesIO(zip_bytes)) as zf:
                zf.extractall(nativemods_path)

            logger.info("Installed native mod: %s", mod_name)
            return True
        except Exception as e:
            logger.error("Failed to install native mod %s: %s", mod_name, e)
            return False

    # --- Helpers ---

    @staticmethod
    def build_environment(server: ServerModel) -> Dict[str, str]:
        # Start with user-defined env vars
        env: Dict[str, str] = {}
        if server.env_vars:
            for k, v in server.env_vars.items():
                env[k] = str(v)

        # Explicit server fields always take precedence over env_vars
        # (these are set via dedicated UI fields, not the ENV accordion)
        env["PORT"] = str(server.game_port)
        env["QUERY_PORT"] = str(server.query_port)
        env["PLAYERS"] = str(server.max_players)
        env["SERVER_NAME"] = server.server_name
        env["SERVER_DESCRIPTION"] = server.server_description
        env["ADMIN_PASSWORD"] = server.admin_password
        env["REST_API_ENABLED"] = "true"
        env["REST_API_PORT"] = str(server.rest_api_port)
        if server.server_password:
            env["SERVER_PASSWORD"] = server.server_password

        return env

    @staticmethod
    def build_port_bindings(server: ServerModel) -> Dict[str, int]:
        return {
            f"{server.game_port}/udp": server.game_port,
            f"{server.query_port}/udp": server.query_port,
            f"{server.rest_api_port}/tcp": server.rest_api_port,
        }

    @staticmethod
    def build_volumes(server: ServerModel) -> Dict[str, Dict[str, str]]:
        volumes = {
            server.data_volume_name: {"bind": "/palworld/", "mode": "rw"},
            server.saves_path: {"bind": "/palworld/Pal/Saved/", "mode": "rw"},
            server.mods_path: {
                "bind": "/palworld/Pal/Binaries/Win64/Mods/",
                "mode": "rw",
            },
            server.logicmods_path: {
                "bind": "/palworld/Pal/Content/Paks/LogicMods/",
                "mode": "rw",
            },
            server.nativemods_path: {
                "bind": "/palworld/nativemods/",
                "mode": "rw",
            },
        }
        return volumes

    @staticmethod
    def suggest_next_ports(allocated: Set[int]) -> Dict[str, int]:
        base_game, base_query, base_rest = 8211, 27015, 8212
        offset = 0
        while True:
            candidate = {
                "game_port": base_game + offset,
                "query_port": base_query + offset,
                "rest_api_port": base_rest + offset,
            }
            if not any(p in allocated for p in candidate.values()):
                return candidate
            offset += 1
