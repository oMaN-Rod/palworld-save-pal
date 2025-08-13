from typing import Dict, List, Optional, Any
from uuid import uuid4
import datetime as dt
from datetime import datetime
from sqlmodel import SQLModel, Field, Relationship
from sqlalchemy import Column, JSON, String, DateTime, Boolean


class UpsCollection(SQLModel, table=True):
    __tablename__ = "ups_collections"

    id: str = Field(default_factory=lambda: str(uuid4()), primary_key=True)
    name: str = Field(sa_column=Column(String, nullable=False))
    description: Optional[str] = Field(default=None, sa_column=Column(String))
    created_at: datetime = Field(
        default_factory=datetime.now(dt.timezone.utc), sa_column=Column(DateTime)
    )

    pals: List["UpsPal"] = Relationship(
        back_populates="collections", link_model="UpsPalCollection"
    )


class UpsPal(SQLModel, table=True):
    __tablename__ = "ups_pals"

    id: str = Field(default_factory=lambda: str(uuid4()), primary_key=True)

    pal_data: Dict[str, Any] = Field(sa_column=Column(JSON, nullable=False))

    # Metadata
    name: str = Field(sa_column=Column(String, nullable=False))  # Display name/nickname
    character_id: str = Field(sa_column=Column(String, nullable=False))  # Pal species
    level: int = Field(default=1, sa_column=Column(String))
    tags: List[str] = Field(default_factory=list, sa_column=Column(JSON))

    source_type: str = Field(
        sa_column=Column(String, nullable=False)
    )  # 'pal_box', 'dps', 'gps'
    source_save_name: Optional[str] = Field(default=None, sa_column=Column(String))
    source_player_name: Optional[str] = Field(default=None, sa_column=Column(String))

    created_at: datetime = Field(
        default_factory=datetime.utcnow, sa_column=Column(DateTime)
    )
    updated_at: datetime = Field(
        default_factory=datetime.utcnow, sa_column=Column(DateTime)
    )

    is_favorite: bool = Field(default=False, sa_column=Column(Boolean))

    collections: List[UpsCollection] = Relationship(
        back_populates="pals", link_model="UpsPalCollection"
    )


class UpsPalCollection(SQLModel, table=True):
    __tablename__ = "ups_pal_collection"

    pal_id: str = Field(foreign_key="ups_pals.id", primary_key=True)
    collection_id: str = Field(foreign_key="ups_collections.id", primary_key=True)
