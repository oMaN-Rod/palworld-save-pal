import os
import re
import shutil
from datetime import datetime
import uuid
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from palworld_save_pal.utils.gamepass.container_types import (
    FILETIME,
    Container,
    ContainerError,
    ContainerFile,
    ContainerFileList,
    ContainerIndex,
)
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

CONTAINER_REGEX = re.compile(r"[0-9A-F]{16}_[0-9A-F]{32}$")
GAMEPASS_PATH = os.path.expandvars(
    r"%LOCALAPPDATA%\Packages\PocketpairInc.Palworld_ad4psfrxyesvt"
)
WGS_PATH = os.path.join(GAMEPASS_PATH, "SystemAppData", "wgs")


def find_container_path() -> Optional[str]:
    """Find the gamepass container directory path"""
    if not os.path.exists(GAMEPASS_PATH):
        raise ContainerError("Could not find Xbox Palworld installation")

    for dir_name in os.listdir(WGS_PATH):
        if CONTAINER_REGEX.match(dir_name):
            return os.path.join(WGS_PATH, dir_name)

    raise ContainerError("Could not find container path. Try running the game once.")


def backup_container_path(container_path: str) -> str:
    """Create a backup of the entire container directory in the application's backup folder"""
    timestamp = datetime.now().strftime("%Y%m%d%H%M%S")
    backup_dir = Path("backups/gamepass")
    backup_dir.mkdir(parents=True, exist_ok=True)

    container_name = os.path.basename(container_path)
    backup_path = backup_dir / f"{container_name}_{timestamp}"

    logger.info("Creating backup of container path: %s", backup_path)
    shutil.copytree(container_path, backup_path)
    return str(backup_path)


def read_container_index(container_path: str) -> ContainerIndex:
    """Read and parse the container index file"""
    index_path = os.path.join(container_path, "containers.index")
    if not os.path.exists(index_path):
        raise ContainerError(f"Container index not found: {index_path}")

    logger.debug("Reading container index from: %s", index_path)
    with open(index_path, "rb") as f:
        return ContainerIndex.from_stream(f)


def get_container_files(
    container_path: str, container: Container
) -> List[ContainerFile]:
    """Get all files from a specific container"""
    container_dir = os.path.join(
        container_path, container.container_uuid.bytes_le.hex().upper()
    )

    files = []
    for filename in os.listdir(container_dir):
        if filename.startswith("container."):
            file_path = os.path.join(container_dir, filename)
            with open(file_path, "rb") as f:
                container_file_list = ContainerFileList.from_stream(f)
                files.extend(container_file_list.files)
    return files


def create_new_container(
    container_path: str,
    save_name: str,
    data: bytes,
    file_name: str = "Data",
    container_suffix: str = "Level",
) -> Container:
    """
    Create a new container with the given save data,
    and ensure we also track which file is "Data".
    """
    container_name = f"{save_name}-{container_suffix}"
    container_uuid = uuid.uuid4()
    container_dir = os.path.join(container_path, container_uuid.bytes_le.hex().upper())

    # Create container file list with unique file UUID
    file_uuid = uuid.uuid4()
    files = [ContainerFile(file_name, file_uuid, data)]

    # Write container list file with explicit format
    os.makedirs(container_dir, exist_ok=True)
    container_file = os.path.join(container_dir, "container.1")
    with open(container_file, "wb") as f:
        # Write container version
        f.write((4).to_bytes(4, "little"))  # Version 4
        f.write(len(files).to_bytes(4, "little"))  # File count

        # Write file entries
        for file in files:
            name_bytes = file.name.encode("utf-16-le")
            name_padding = b"\0" * (128 - len(name_bytes))  # 64 chars * 2 bytes
            f.write(name_bytes + name_padding)
            # Cloud UUID (zeros)
            f.write(b"\0" * 16)
            # Actual file UUID
            f.write(file.uuid.bytes)

            # Write file data
            file_path = os.path.join(container_dir, file.uuid.bytes_le.hex().upper())
            with open(file_path, "wb") as data_file:
                data_file.write(file.data)

    logger.info("Created new container with UUID: %s", container_uuid)
    logger.info("Created file with UUID: %s", file_uuid)

    container = Container(
        container_name=container_name,
        cloud_id="",
        seq=1,
        flag=5,  # typical save flag
        container_uuid=container_uuid,
        mtime=FILETIME.from_timestamp(datetime.now().timestamp()),
        size=len(data),
    )

    return container


def copy_container(
    source_container: Container,
    source_path: str,
    dest_path: str,
    new_save_name: str,
) -> Container:
    """
    Copy an existing container with a new UUID and name,
    then set container.file if needed.
    """
    source_dir = os.path.join(
        source_path, source_container.container_uuid.bytes_le.hex().upper()
    )
    source_files: List[ContainerFile] = []
    for filename in os.listdir(source_dir):
        if filename.startswith("container."):
            with open(os.path.join(source_dir, filename), "rb") as f:
                file_list = ContainerFileList.from_stream(f)
                source_files.extend(file_list.files)

    new_container_uuid = uuid.uuid4()
    new_container_name = source_container.container_name.replace(
        source_container.container_name.split("-")[0], new_save_name
    )

    new_container_dir = os.path.join(
        dest_path, new_container_uuid.bytes_le.hex().upper()
    )
    os.makedirs(new_container_dir, exist_ok=True)

    new_files: List[ContainerFile] = []
    for file in source_files:
        new_file_uuid = uuid.uuid4()
        new_files.append(ContainerFile(file.name, new_file_uuid, file.data))

        file_path = os.path.join(
            new_container_dir, new_file_uuid.bytes_le.hex().upper()
        )
        with open(file_path, "wb") as f:
            f.write(file.data)
        logger.info("Created new file UUID: %s", new_file_uuid)

    # Now create the new container.1
    container_file = os.path.join(new_container_dir, "container.1")
    with open(container_file, "wb") as f:
        f.write((4).to_bytes(4, "little"))  # version
        f.write(len(new_files).to_bytes(4, "little"))  # file_count
        for file in new_files:
            name_bytes = file.name.encode("utf-16-le")
            name_padding = b"\0" * (128 - len(name_bytes))
            f.write(name_bytes + name_padding)
            f.write(b"\0" * 16)
            f.write(file.uuid.bytes)
            logger.debug("Wrote new file UUID: %s", file.uuid)

    new_container = Container(
        container_name=new_container_name,
        cloud_id="",
        seq=1,
        flag=5,
        container_uuid=new_container_uuid,
        mtime=FILETIME.from_timestamp(datetime.now().timestamp()),
        size=sum(len(f.data) for f in new_files),
    )

    logger.info("Copied container with UUID: %s", source_container.container_uuid)
    return new_container


def save_modified_gamepass(
    container_path: str,
    save_name: str,
    modified_level_data: bytes,
    original_containers: Dict[str, Container],
) -> None:
    """
    Save modified gamepass save: create a new container for the Level,
    then copy other containers (LevelMeta, LocalData, etc.) so the new
    containers all have file paths as well.
    """
    logger.info("Saving modified gamepass save: %s", save_name)

    # Read existing container index
    container_index = read_container_index(container_path)

    # Create new container for modified Level.sav
    new_level_container = create_new_container(
        container_path, save_name, modified_level_data
    )
    container_index.containers.append(new_level_container)

    # Copy other containers
    for suffix, original_container in original_containers.items():
        if suffix == "Level" or suffix.isdigit():
            continue
        logger.debug("Copying container: %s", original_container.container_name)
        new_container = copy_container(
            original_container,
            container_path,
            container_path,
            save_name,
        )
        container_index.containers.append(new_container)

        # (Similarly, you can read its container.1 and set .file)

    # Update container index timestamp
    container_index.mtime = FILETIME.from_timestamp(datetime.now().timestamp())

    # Write updated container index
    container_index.write_file(container_path)
    logger.info("Successfully saved modified gamepass save: %s", save_name)


def get_save_containers(
    container_index: ContainerIndex, save_name: str
) -> Dict[str, Container]:
    """Get all containers for a specific save"""
    containers = {}
    for container in container_index.containers:
        if container.container_name.startswith(f"{save_name}-"):
            suffix = container.container_name.split("-")[-1]
            if not container.container_name.startswith(f"{save_name}-Players-"):
                containers[suffix] = container
            else:
                player_id = container.container_name.split("-")[-1]
                containers[f"Players-{player_id}"] = container
    return containers
