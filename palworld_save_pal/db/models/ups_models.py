from datetime import datetime
from typing import Any, Dict, List, Optional
from uuid import UUID, uuid4

from sqlmodel import Field, SQLModel, JSON, Column, Relationship


class UPSPalModel(SQLModel, table=True):
    __tablename__ = "ups_pals"

    id: int = Field(default=None, primary_key=True)
    instance_id: UUID = Field(default_factory=uuid4, unique=True, index=True)
    character_id: str = Field(index=True)  # e.g., "SheepBall", "BlackGriffon"
    nickname: Optional[str] = Field(default=None, index=True)
    level: int = Field(default=1, index=True)
    pal_data: Dict[str, Any] = Field(sa_column=Column(JSON))

    # metadata
    source_save_file: Optional[str] = Field(default=None)
    source_player_uid: Optional[UUID] = Field(default=None)
    source_player_name: Optional[str] = Field(default=None)
    source_storage_type: Optional[str] = Field(default=None)  # "pal_box", "gps", "dps"
    source_storage_slot: Optional[int] = Field(default=None)

    collection_id: Optional[int] = Field(
        default=None, foreign_key="ups_collections.id", index=True
    )
    tags: List[str] = Field(default_factory=list, sa_column=Column(JSON))
    notes: Optional[str] = Field(default=None)

    created_at: datetime = Field(default_factory=datetime.utcnow, index=True)
    updated_at: datetime = Field(default_factory=datetime.utcnow)
    last_accessed_at: Optional[datetime] = Field(default=None)

    transfer_count: int = Field(default=0)
    clone_count: int = Field(default=0)

    collection: Optional["UPSCollectionModel"] = Relationship(back_populates="pals")


class UPSCollectionModel(SQLModel, table=True):
    __tablename__ = "ups_collections"

    id: int = Field(default=None, primary_key=True)
    name: str = Field(index=True)
    description: Optional[str] = Field(default=None)
    color: Optional[str] = Field(default=None)
    icon: Optional[str] = Field(default=None)

    is_favorite: bool = Field(default=False, index=True)
    is_archived: bool = Field(default=False, index=True)

    pal_count: int = Field(default=0)

    created_at: datetime = Field(default_factory=datetime.utcnow, index=True)
    updated_at: datetime = Field(default_factory=datetime.utcnow)

    pals: List["UPSPalModel"] = Relationship(back_populates="collection")


class UPSTagModel(SQLModel, table=True):
    __tablename__ = "ups_tags"

    id: int = Field(default=None, primary_key=True)
    name: str = Field(unique=True, index=True)
    description: Optional[str] = Field(default=None)
    color: Optional[str] = Field(default=None)

    usage_count: int = Field(default=0)
    created_at: datetime = Field(default_factory=datetime.utcnow)
    updated_at: datetime = Field(default_factory=datetime.utcnow)


class UPSStatsModel(SQLModel, table=True):
    __tablename__ = "ups_stats"

    id: int = Field(default=1, primary_key=True)

    total_pals: int = Field(default=0)
    total_collections: int = Field(default=0)
    total_tags: int = Field(default=0)
    total_transfers: int = Field(default=0)
    total_clones: int = Field(default=0)

    storage_size_mb: float = Field(default=0.0)

    most_transferred_pal_id: Optional[int] = Field(default=None)
    most_cloned_pal_id: Optional[int] = Field(default=None)
    most_popular_character_id: Optional[str] = Field(default=None)

    element_distribution: str = Field(default="{}")
    alpha_count: int = Field(default=0)
    lucky_count: int = Field(default=0)
    human_count: int = Field(default=0)
    predator_count: int = Field(default=0)
    oilrig_count: int = Field(default=0)
    summon_count: int = Field(default=0)

    last_updated: datetime = Field(default_factory=datetime.utcnow)


class UPSTransferLogModel(SQLModel, table=True):
    __tablename__ = "ups_transfer_log"

    id: int = Field(default=None, primary_key=True)
    pal_id: int = Field(foreign_key="ups_pals.id", index=True)
    operation_type: str = Field(index=True)  # "import", "export", "clone", "delete"

    source_type: Optional[str] = Field(default=None)  # "ups", "pal_box", "gps", "dps"
    source_location: Optional[str] = Field(default=None)
    destination_type: Optional[str] = Field(default=None)
    destination_location: Optional[str] = Field(default=None)

    # metadata
    save_file_name: Optional[str] = Field(default=None)
    player_name: Optional[str] = Field(default=None)
    player_uid: Optional[UUID] = Field(default=None)

    success: bool = Field(default=True, index=True)
    error_message: Optional[str] = Field(default=None)

    timestamp: datetime = Field(default_factory=datetime.utcnow, index=True)
