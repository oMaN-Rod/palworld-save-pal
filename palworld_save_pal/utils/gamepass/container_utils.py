import os
import re
import shutil
from datetime import datetime
import uuid
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.game.save_file import SaveFile
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
    if not os.path.exists(GAMEPASS_PATH):
        raise ContainerError("Could not find Xbox Palworld installation")

    for dir_name in os.listdir(WGS_PATH):
        if CONTAINER_REGEX.match(dir_name):
            return os.path.join(WGS_PATH, dir_name)

    raise ContainerError("Could not find container path. Try running the game once.")


def backup_container_path(container_path: str) -> str:
    timestamp = datetime.now().strftime("%Y%m%d%H%M%S")
    backup_dir = Path("backups/gamepass")
    backup_dir.mkdir(parents=True, exist_ok=True)

    container_name = os.path.basename(container_path)
    backup_path = backup_dir / f"{container_name}_{timestamp}"

    logger.info("Creating backup of container path: %s", backup_path)
    shutil.copytree(container_path, backup_path)
    return str(backup_path)


def cleanup_container_path(
    container_index: ContainerIndex, container_path: str
) -> None:
    for root, dirs, files in os.walk(container_path, topdown=False):
        directory_name = os.path.basename(root)
        if "_" in directory_name:
            continue
        should_remove = False

        # Check if directory is empty
        if len(files) == 0 and len(dirs) == 0:
            logger.debug("Empty directory found: %s", directory_name)
            should_remove = True

        # Check container file list
        container_file = next((f for f in files if f.startswith("container.")), None)
        if container_file:
            with open(os.path.join(root, container_file), "rb") as f:
                file_list = ContainerFileList.from_stream(f)
                if len(file_list.files) == 0:
                    logger.debug(
                        "Empty container file list: %s/%s",
                        directory_name,
                        container_file,
                    )
                    should_remove = True

        # Find matching container in index
        matching_container = next(
            (
                c
                for c in container_index.containers
                if directory_name == c.container_uuid.bytes_le.hex().upper()
            ),
            None,
        )

        # If no matching container found, remove directory
        if not matching_container:
            logger.debug(
                "No matching container found for directory: %s",
                directory_name,
            )
            shutil.rmtree(root)
            continue

        # Remove marked directories and their container index entries
        if should_remove:
            logger.debug("Purging container: %s", matching_container.container_name)
            container_index.containers.remove(matching_container)
            shutil.rmtree(root)


def read_container_index(container_path: str) -> ContainerIndex:
    index_path = os.path.join(container_path, "containers.index")
    if not os.path.exists(index_path):
        raise ContainerError(f"Container index not found: {index_path}")

    logger.debug("Reading container index from: %s", index_path)
    with open(index_path, "rb") as f:
        return ContainerIndex.from_stream(f)


def create_new_container(
    container_path: str,
    save_name: str,
    data: bytes,
    file_name: str = "Data",
    container_suffix: str = "Level",
) -> Container:
    container_name = f"{save_name}-{container_suffix}"
    container_uuid = uuid.uuid4()
    container_dir = os.path.join(container_path, container_uuid.bytes_le.hex().upper())

    # Create container file list with unique file UUID
    file_uuid = uuid.uuid4()

    # Write container list file with explicit format
    os.makedirs(container_dir, exist_ok=True)
    container_file = os.path.join(container_dir, "container.1")
    with open(container_file, "wb") as f:
        # Write container version
        f.write((4).to_bytes(4, "little"))  # Version 4
        f.write((1).to_bytes(4, "little"))  # File count

        # Write file entries
        name_bytes = file_name.encode("utf-16-le")
        name_padding = b"\0" * (128 - len(name_bytes))  # 64 chars * 2 bytes
        f.write(name_bytes + name_padding)
        # Cloud UUID (zeros)
        f.write(b"\0" * 16)
        # Actual file UUID
        f.write(file_uuid.bytes)

        # Write file data
        file_path = os.path.join(container_dir, file_uuid.bytes_le.hex().upper())
        with open(file_path, "wb") as data_file:
            data_file.write(data)

    logger.info(
        "Created new container with; uuid=%s, file_uuid=%s",
        container_uuid.bytes.hex().upper(),
        file_uuid.bytes.hex().upper(),
    )

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


def clean_file_name(input_str):
    # Remove -Slot[number]- pattern
    temp_str = re.sub(r"-Slot\d*-", "-", input_str)

    # Remove trailing -[number] pattern
    result = re.sub(r"-(\d{2})$", "", temp_str)
    logger.debug("Cleaned file name: %s => %s", input_str, result)
    return result


def copy_container(
    source_container: Container,
    source_path: str,
    dest_path: str,
    new_save_id: str,
    key: str,
) -> Container:
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
        source_container.container_name.split("-")[0], new_save_id
    )

    new_container_dir = os.path.join(
        dest_path, new_container_uuid.bytes_le.hex().upper()
    )
    os.makedirs(new_container_dir, exist_ok=True)

    new_files: List[ContainerFile] = []
    for file in source_files:
        new_file_uuid = uuid.uuid4()
        if key == "LevelMeta":
            level_meta = SaveFile().load_level_meta(file.data)
            world_name = PalObjects.get_nested(
                level_meta.properties, "SaveData", "value", "WorldName", "value"
            )
            current_timestamp = f"PSP-{datetime.now().strftime('%Y-%m-%d_%H:%M')}"

            if match := re.search(
                r"(.*?)\s*PSP-\d{4}-\d{2}-\d{2}_\d{2}:\d{2}\s*$", world_name
            ):
                base_name = match.group(1).strip()
                world_name = f"{base_name} {current_timestamp}"
            else:
                world_name = f"{world_name.strip()} {current_timestamp}"

            level_meta.properties["SaveData"]["value"]["WorldName"][
                "value"
            ] = world_name
            file.data = SaveFile().sav(level_meta)

        file_name = clean_file_name(file.name)
        new_files.append(ContainerFile(file_name, new_file_uuid, file.data))

        file_path = os.path.join(
            new_container_dir, new_file_uuid.bytes_le.hex().upper()
        )
        with open(file_path, "wb") as f:
            f.write(file.data)
        logger.debug("Created new file: %s", file_path)

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
    logger.debug("Created new container file: %s", container_file)

    new_container = Container(
        container_name=new_container_name,
        cloud_id="",
        seq=1,
        flag=5,
        container_uuid=new_container_uuid,
        mtime=FILETIME.from_timestamp(datetime.now().timestamp()),
        size=sum(len(f.data) for f in new_files),
    )

    logger.debug(
        "Copied container: %s => %s",
        new_container_name,
        source_container.container_uuid,
    )
    return new_container


def save_modified_gamepass(
    container_index: ContainerIndex,
    container_path: str,
    save_id: str,
    modified_level_data: bytes,
    original_containers: Dict[str, Container],
) -> None:
    logger.info("Saving modified gamepass save: %s", save_id)

    # Create new container for modified Level.sav
    new_level_container = create_new_container(
        container_path, save_id, modified_level_data
    )
    container_index.containers.append(new_level_container)

    # Copy other containers
    for key, original_container in original_containers.items():
        if key == "Level":
            continue
        logger.debug("Copying container: %s", original_container.container_name)
        new_container = copy_container(
            original_container, container_path, container_path, save_id, key
        )
        container_index.containers.append(new_container)

    # Update container index timestamp
    container_index.mtime = FILETIME.from_timestamp(datetime.now().timestamp())

    # Write updated container index
    container_index.write_file(container_path)
    logger.info("Successfully saved modified gamepass save: %s", save_id)
