from datetime import datetime
from typing import Any, Dict, Optional

from sqlmodel import Field, SQLModel, JSON, Column


class ServerModel(SQLModel, table=True):
    __tablename__ = "servers"

    id: int = Field(default=None, primary_key=True)
    name: str = Field(index=True)
    container_name: str = Field(unique=True, index=True)
    image_name: str = Field(default="omanrod/psp-palworld-server")

    # Deployment type: "docker" or "native"
    server_type: str = Field(default="docker")

    # Networking
    game_port: int = Field(default=8211)
    query_port: int = Field(default=27015)
    rest_api_port: int = Field(default=8212)

    # Paths
    data_volume_name: str = Field(default="")
    saves_path: str = Field(default="")
    mods_path: str = Field(default="")
    logicmods_path: str = Field(default="")
    nativemods_path: str = Field(default="")

    # Native server fields
    install_path: str = Field(default="")
    steamcmd_path: str = Field(default="")
    pid: Optional[int] = Field(default=None)
    launch_args: str = Field(default="")
    workshop_dir: str = Field(default="")

    # Server identity
    server_name: str = Field(default="PSP Palworld Server")
    server_description: str = Field(default="")
    server_password: str = Field(default="")
    admin_password: str = Field(default="admin")
    max_players: int = Field(default=16)

    # All ENV vars as a single JSON blob
    env_vars: Dict[str, Any] = Field(default_factory=dict, sa_column=Column(JSON))

    # Metadata
    created_at: datetime = Field(default_factory=datetime.utcnow)
    updated_at: datetime = Field(default_factory=datetime.utcnow)
