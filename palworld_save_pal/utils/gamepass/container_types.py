from datetime import datetime, timezone
import io
import os
import uuid
from typing import BinaryIO, Dict, List, Optional

from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

FILETIME_EPOCH = datetime(1601, 1, 1, tzinfo=timezone.utc)
CONTAINER_INDEX_VERSION = 0xE
CONTAINER_FILE_VERSION = 4


class GamepassError(Exception):
    """Base exception for gamepass-related errors"""

    pass


class ContainerError(GamepassError):
    """Raised when container operations fail"""

    pass


class FILETIME:
    """Windows FILETIME representation for container timestamps"""

    def __init__(self, value: int):
        self.value = value

    @classmethod
    def from_stream(cls, stream: BinaryIO) -> "FILETIME":
        """Read FILETIME from binary stream"""
        return cls(int.from_bytes(stream.read(8), "little"))

    @classmethod
    def from_timestamp(cls, timestamp: float) -> "FILETIME":
        """Convert Unix timestamp to FILETIME"""
        return cls(int(timestamp * 10000000 + 116444736000000000))

    def to_bytes(self) -> bytes:
        """Convert to bytes for writing to stream"""
        return self.value.to_bytes(8, "little")

    def to_timestamp(self) -> float:
        """Convert to Unix timestamp"""
        return (self.value - 116444736000000000) / 10000000

    def __eq__(self, other: "FILETIME") -> bool:
        """Equal to comparison"""
        if not isinstance(other, FILETIME):
            return NotImplemented
        return self.value == other.value

    def __lt__(self, other: "FILETIME") -> bool:
        """Less than comparison"""
        if not isinstance(other, FILETIME):
            return NotImplemented
        return self.value < other.value

    def __gt__(self, other: "FILETIME") -> bool:
        """Greater than comparison"""
        if not isinstance(other, FILETIME):
            return NotImplemented
        return self.value > other.value

    def __le__(self, other: "FILETIME") -> bool:
        """Less than or equal to comparison"""
        if not isinstance(other, FILETIME):
            return NotImplemented
        return self.value <= other.value

    def __ge__(self, other: "FILETIME") -> bool:
        """Greater than or equal to comparison"""
        if not isinstance(other, FILETIME):
            return NotImplemented
        return self.value >= other.value


class ContainerFile:
    """Represents a file within a container"""

    def __init__(self, name: str, file_uuid: uuid.UUID, data: bytes):
        self.name = name
        self.uuid = file_uuid
        self.data = data

    def __repr__(self):
        return f"ContainerFile(name={self.name}, uuid={self.uuid})"


class ContainerFileList:
    """Represents a list of files in a container"""

    def __init__(self, *, seq: int, files: List[ContainerFile]):
        self.seq = seq
        self.files = files

    @classmethod
    def from_stream(cls, stream: BinaryIO) -> "ContainerFileList":
        """Read ContainerFileList from binary stream"""
        try:
            seq = int(os.path.splitext(os.path.basename(stream.name))[1][1:])
        except ValueError:
            raise ContainerError(f"Invalid container file name: {stream.name}")

        path = os.path.dirname(stream.name)
        version = int.from_bytes(stream.read(4), "little")

        if version != CONTAINER_FILE_VERSION:
            raise ContainerError(f"Unsupported container file version: {version}")

        file_count = int.from_bytes(stream.read(4), "little")
        files = []

        for _ in range(file_count):
            # Read file entry
            file_name = cls._read_utf16_fixed_string(stream, 64)
            # Skip cloud UUID
            stream.read(16)
            file_uuid = uuid.UUID(bytes=stream.read(16))

            # Read file data
            file_path = os.path.join(path, file_uuid.bytes_le.hex().upper())
            if not os.path.exists(file_path):
                raise ContainerError(f"File does not exist: {file_path}")

            with open(file_path, "rb") as f:
                file_data = f.read()

            files.append(ContainerFile(file_name, file_uuid, file_data))

        return cls(seq=seq, files=files)

    def write_container(self, path: str):
        """Write ContainerFileList and its files to disk"""
        os.makedirs(path, exist_ok=True)

        # Write container list file
        container_path = os.path.join(path, f"container.{self.seq}")
        with open(container_path, "wb") as f:
            # Write header
            f.write(CONTAINER_FILE_VERSION.to_bytes(4, "little"))
            f.write(len(self.files).to_bytes(4, "little"))

            # Write file entries
            for file in self.files:
                self._write_utf16_fixed_string(f, file.name, 64)
                f.write(b"\0" * 16)  # Empty cloud UUID
                f.write(file.uuid.bytes)

                # Write file data
                file_path = os.path.join(path, file.uuid.bytes_le.hex().upper())
                with open(file_path, "wb") as data_file:
                    data_file.write(file.data)

    @staticmethod
    def _read_utf16_fixed_string(stream: BinaryIO, length: int) -> str:
        """Read fixed-length UTF-16 string from stream"""
        return stream.read(length * 2).decode("utf-16-le").rstrip("\0")

    @staticmethod
    def _write_utf16_fixed_string(stream: BinaryIO, value: str, length: int):
        """Write fixed-length UTF-16 string to stream"""
        encoded = value.encode("utf-16-le")
        padding = b"\0" * (length * 2 - len(encoded))
        stream.write(encoded + padding)


class Container:
    """Represents a single container in the gamepass save system"""

    def __init__(
        self,
        *,
        container_name: str,
        cloud_id: str,
        seq: int,
        flag: int,
        container_uuid: uuid.UUID,
        mtime: FILETIME,
        size: int,
    ):
        self.container_name = container_name
        self.cloud_id = cloud_id
        self.seq = seq
        self.flag = flag
        self.container_uuid = container_uuid
        self.mtime = mtime
        self.size = size

    @classmethod
    def from_stream(cls, stream: BinaryIO) -> "Container":
        """Read Container from binary stream"""
        # Read and validate container name
        container_name = cls._read_utf16_string(stream)
        container_name_repeated = cls._read_utf16_string(stream)
        if container_name != container_name_repeated:
            raise ContainerError(
                f"Container name mismatch: {container_name} != {container_name_repeated}"
            )

        # Read container metadata
        cloud_id = cls._read_utf16_string(stream)
        seq = int.from_bytes(stream.read(1), "little")
        flag = int.from_bytes(stream.read(4), "little")

        # Validate cloud ID and flag relationship
        if (cloud_id == "" and flag & 4 == 0) or (cloud_id != "" and flag & 4 != 0):
            raise ContainerError(f"Mismatch between cloud id and flag state")

        # Read remaining fields
        container_uuid = uuid.UUID(bytes=stream.read(16))
        mtime = FILETIME.from_stream(stream)

        # Skip reserved bytes
        reserved = int.from_bytes(stream.read(8), "little")
        if reserved != 0:
            logger.warning(f"Unexpected non-zero reserved bytes: {reserved}")

        size = int.from_bytes(stream.read(8), "little")

        return cls(
            container_name=container_name,
            cloud_id=cloud_id,
            seq=seq,
            flag=flag,
            container_uuid=container_uuid,
            mtime=mtime,
            size=size,
        )

    def to_bytes(self) -> bytes:
        """Convert Container to bytes for writing"""
        output = io.BytesIO()
        self._write_utf16_string(output, self.container_name)
        self._write_utf16_string(output, self.container_name)
        self._write_utf16_string(output, self.cloud_id)
        output.write(self.seq.to_bytes(1, "little"))
        output.write(self.flag.to_bytes(4, "little"))
        output.write(self.container_uuid.bytes)
        output.write(self.mtime.to_bytes())
        output.write((0).to_bytes(8, "little"))  # Reserved bytes
        output.write(self.size.to_bytes(8, "little"))
        return output.getvalue()

    @staticmethod
    def _read_utf16_string(stream: BinaryIO) -> str:
        """Read UTF-16 string with length prefix from stream"""
        length = int.from_bytes(stream.read(4), "little")
        if length == 0:
            return ""
        return stream.read(length * 2).decode("utf-16-le")

    @staticmethod
    def _write_utf16_string(stream: BinaryIO, value: str):
        """Write UTF-16 string with length prefix to stream"""
        stream.write(len(value).to_bytes(4, "little"))
        stream.write(value.encode("utf-16-le"))


class ContainerIndex:
    """Represents the main container index file"""

    def __init__(
        self,
        *,
        flag1: int,
        package_name: str,
        mtime: FILETIME,
        flag2: int,
        index_uuid: str,
        unknown: int,
        containers: List[Container],
    ):
        self.flag1 = flag1
        self.package_name = package_name
        self.mtime = mtime
        self.flag2 = flag2
        self.index_uuid = index_uuid
        self.unknown = unknown
        self.containers = containers

    @classmethod
    def from_stream(cls, stream: BinaryIO) -> "ContainerIndex":
        """Read ContainerIndex from binary stream"""
        version = int.from_bytes(stream.read(4), "little")
        if version != CONTAINER_INDEX_VERSION:
            raise ContainerError(f"Unsupported container index version: {version}")

        file_count = int.from_bytes(stream.read(4), "little")
        flag1 = int.from_bytes(stream.read(4), "little")

        # Read package info
        package_name = cls._read_utf16_string(stream)
        mtime = FILETIME.from_stream(stream)
        flag2 = int.from_bytes(stream.read(4), "little")
        index_uuid = cls._read_utf16_string(stream)
        unknown = int.from_bytes(stream.read(8), "little")

        # Read containers
        containers = []
        for _ in range(file_count):
            containers.append(Container.from_stream(stream))

        return cls(
            flag1=flag1,
            package_name=package_name,
            mtime=mtime,
            flag2=flag2,
            index_uuid=index_uuid,
            unknown=unknown,
            containers=containers,
        )

    def get_save_containers(self, save_name: str) -> Dict[str, Container]:
        latest_containers: Dict[str, Container] = {}

        for container in self.containers:
            if not container.container_name.startswith(f"{save_name}-"):
                continue

            # Handle player containers
            if "Players-" in container.container_name:
                player_id = container.container_name.split("Players-")[-1]
                key = f"Players-{player_id}"
            # Handle other container types
            else:
                # Map container name to standardized key
                if "LocalData" in container.container_name:
                    key = "LocalData"
                elif "LevelMeta" in container.container_name:
                    key = "LevelMeta"
                elif "Level" in container.container_name:
                    key = "Level"
                elif "WorldOption" in container.container_name:
                    key = "WorldOption"
                else:
                    continue  # Skip containers that don't match our expected types

            # Update if this container is more recent
            if key not in latest_containers or (
                container.seq > latest_containers[key].seq
                or (
                    container.seq == latest_containers[key].seq
                    and container.mtime > latest_containers[key].mtime
                )
            ):
                latest_containers[key] = container

        return latest_containers

    def write_file(self, path: str):
        """Write ContainerIndex to disk"""
        index_path = os.path.join(path, "containers.index")
        with open(index_path, "wb") as f:
            # Write header
            f.write(CONTAINER_INDEX_VERSION.to_bytes(4, "little"))
            f.write(len(self.containers).to_bytes(4, "little"))
            f.write(self.flag1.to_bytes(4, "little"))

            # Write package info
            self._write_utf16_string(f, self.package_name)
            f.write(self.mtime.to_bytes())
            f.write(self.flag2.to_bytes(4, "little"))
            self._write_utf16_string(f, self.index_uuid)
            f.write(self.unknown.to_bytes(8, "little"))

            # Write containers
            for container in self.containers:
                f.write(container.to_bytes())

    @staticmethod
    def _read_utf16_string(stream: BinaryIO) -> str:
        """Read UTF-16 string with length prefix"""
        length = int.from_bytes(stream.read(4), "little")
        if length == 0:
            return ""
        return stream.read(length * 2).decode("utf-16-le")

    @staticmethod
    def _write_utf16_string(stream: BinaryIO, value: str):
        """Write UTF-16 string with length prefix"""
        stream.write(len(value).to_bytes(4, "little"))
        stream.write(value.encode("utf-16-le"))
