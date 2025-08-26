import json
import datetime as dt
from datetime import datetime
from typing import Dict, List, Optional, Tuple, Any
from uuid import UUID

from sqlmodel import Session, select, func, and_, or_
from sqlalchemy import desc, asc

from palworld_save_pal.db.bootstrap import engine
from palworld_save_pal.db.models.ups_models import (
    UPSPalModel,
    UPSCollectionModel,
    UPSTagModel,
    UPSStatsModel,
    UPSTransferLogModel,
)
from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.json_manager import JsonManager

logger = create_logger(__name__)


class UPSService:
    @staticmethod
    def add_pal(
        pal_dto: PalDTO,
        source_save_file: Optional[str] = None,
        source_player_uid: Optional[UUID] = None,
        source_player_name: Optional[str] = None,
        source_storage_type: Optional[str] = None,
        source_storage_slot: Optional[int] = None,
        collection_id: Optional[int] = None,
        tags: Optional[List[str]] = None,
        notes: Optional[str] = None,
    ) -> UPSPalModel:
        with Session(engine) as session:
            pal_data = pal_dto.model_dump(mode="json")

            ups_pal = UPSPalModel(
                character_id=pal_dto.character_id,
                nickname=pal_dto.nickname,
                level=pal_dto.level,
                pal_data=pal_data,
                source_save_file=source_save_file,
                source_player_uid=source_player_uid,
                source_player_name=source_player_name,
                source_storage_type=source_storage_type,
                source_storage_slot=source_storage_slot,
                collection_id=collection_id,
                tags=tags or [],
                notes=notes,
            )

            session.add(ups_pal)
            session.commit()
            session.refresh(ups_pal)

            # Update statistics and collection counts
            UPSService._update_stats(session)
            UPSService._update_collection_counts(session)

            # Log transfer
            UPSService._log_transfer(
                session=session,
                pal_id=ups_pal.id,
                operation_type="import",
                source_type=source_storage_type,
                destination_type="ups",
                save_file_name=source_save_file,
                player_name=source_player_name,
                player_uid=source_player_uid,
                success=True,
            )

            # Ensure all attributes are loaded before expunging
            # Access all the attributes that will be needed after expunging
            _ = ups_pal.id
            _ = ups_pal.instance_id
            _ = ups_pal.character_id
            _ = ups_pal.nickname
            _ = ups_pal.level
            _ = ups_pal.collection_id
            _ = ups_pal.tags
            _ = ups_pal.notes
            _ = ups_pal.created_at
            _ = ups_pal.updated_at
            _ = ups_pal.last_accessed_at
            _ = ups_pal.transfer_count
            _ = ups_pal.clone_count
            _ = ups_pal.pal_data
            _ = ups_pal.source_save_file
            _ = ups_pal.source_player_uid
            _ = ups_pal.source_player_name
            _ = ups_pal.source_storage_type
            _ = ups_pal.source_storage_slot
            _ = ups_pal.collection
            _ = ups_pal.tags
            _ = ups_pal.notes

            # Create a detached copy with all necessary attributes
            session.expunge(ups_pal)
            return ups_pal

    @staticmethod
    def get_all_filtered_pal_ids(
        search_query: Optional[str] = None,
        character_id_filter: Optional[str] = None,
        collection_id: Optional[int] = None,
        tags: Optional[List[str]] = None,
        element_types: Optional[List[str]] = None,
        pal_types: Optional[List[str]] = None,
    ) -> List[int]:
        """Get all pal IDs matching the current filters without pagination."""
        with Session(engine) as session:
            query = select(UPSPalModel.id)

            # Apply the same filters as get_pals
            conditions = []

            if search_query:
                search_condition = or_(
                    UPSPalModel.character_id.ilike(f"%{search_query}%"),
                    UPSPalModel.nickname.ilike(f"%{search_query}%"),
                    UPSPalModel.notes.ilike(f"%{search_query}%"),
                )
                conditions.append(search_condition)

            if character_id_filter and character_id_filter != "All":
                conditions.append(UPSPalModel.character_id == character_id_filter)

            if collection_id is not None:
                conditions.append(UPSPalModel.collection_id == collection_id)

            if tags:
                for tag in tags:
                    tag_json = json.dumps(tag)
                    conditions.append(UPSPalModel.tags.like(f"%{tag_json}%"))

            # Filter by element types (need to look up from pals.json data)
            if element_types:
                # Load pals data to get element types for each character
                pals_json = JsonManager("data/json/pals.json")
                pals_data = pals_json.read()

                # Find all character_ids that have the requested element types
                matching_character_ids = []
                for code_name, pal_info in pals_data.items():
                    pal_element_types = pal_info.get("element_types", [])
                    # Check if any of the requested element types match this pal
                    if any(element in pal_element_types for element in element_types):
                        matching_character_ids.append(code_name)

                # Add condition to filter by these character_ids
                if matching_character_ids:
                    character_conditions = []
                    for char_id in matching_character_ids:
                        character_conditions.append(UPSPalModel.character_id == char_id)
                    conditions.append(or_(*character_conditions))

            # Filter by pal types (alpha, lucky, human, predator, oilrig, summon)
            if pal_types:
                type_conditions = []
                for pal_type in pal_types:
                    if pal_type == "alpha":
                        # Check for is_boss: true in pal_data
                        type_conditions.append(
                            UPSPalModel.pal_data.like('%"is_boss": true%')
                        )
                    elif pal_type == "lucky":
                        # Check for is_lucky: true in pal_data
                        type_conditions.append(
                            UPSPalModel.pal_data.like('%"is_lucky": true%')
                        )
                    elif pal_type == "human":
                        # Humans are identified by is_pal: false in pals.json
                        pals_json = JsonManager("data/json/pals.json")
                        pals_data = pals_json.read()
                        human_character_ids = [
                            code_name
                            for code_name, pal_info in pals_data.items()
                            if not pal_info.get("is_pal", True)
                        ]
                        if human_character_ids:
                            human_conditions = []
                            for char_id in human_character_ids:
                                human_conditions.append(
                                    UPSPalModel.character_id == char_id
                                )
                            type_conditions.append(or_(*human_conditions))
                    elif pal_type == "predator":
                        # Check character_id contains "predator_"
                        type_conditions.append(
                            UPSPalModel.character_id.like("%predator_%")
                        )
                    elif pal_type == "oilrig":
                        # Check character_id contains "_oilrig"
                        type_conditions.append(
                            UPSPalModel.character_id.like("%_oilrig%")
                        )
                    elif pal_type == "summon":
                        # Check character_id contains "summon_"
                        type_conditions.append(
                            UPSPalModel.character_id.like("%summon_%")
                        )
                if type_conditions:
                    conditions.append(or_(*type_conditions))

            if conditions:
                query = query.where(and_(*conditions))

            # Get all IDs
            pal_ids = session.exec(query).all()
            return list(pal_ids)

    @staticmethod
    def get_pals(
        offset: int = 0,
        limit: int = 30,
        search_query: Optional[str] = None,
        character_id_filter: Optional[str] = None,
        collection_id: Optional[int] = None,
        tags: Optional[List[str]] = None,
        element_types: Optional[List[str]] = None,
        pal_types: Optional[List[str]] = None,
        sort_by: str = "created_at",
        sort_order: str = "desc",
    ) -> Tuple[List[UPSPalModel], int]:
        with Session(engine) as session:
            query = select(UPSPalModel)

            # Apply filters
            conditions = []

            if search_query:
                search_condition = or_(
                    UPSPalModel.character_id.ilike(f"%{search_query}%"),
                    UPSPalModel.nickname.ilike(f"%{search_query}%"),
                    UPSPalModel.notes.ilike(f"%{search_query}%"),
                )
                conditions.append(search_condition)

            if character_id_filter and character_id_filter != "All":
                conditions.append(UPSPalModel.character_id == character_id_filter)

            if collection_id is not None:
                conditions.append(UPSPalModel.collection_id == collection_id)

            if tags:
                for tag in tags:
                    tag_json = json.dumps(tag)
                    conditions.append(UPSPalModel.tags.like(f"%{tag_json}%"))

            # Filter by element types (need to look up from pals.json data)
            if element_types:
                # Load pals data to get element types for each character
                pals_json = JsonManager("data/json/pals.json")
                pals_data = pals_json.read()

                # Find all character_ids that have the requested element types
                matching_character_ids = []
                for code_name, pal_info in pals_data.items():
                    pal_element_types = pal_info.get("element_types", [])
                    # Check if any of the requested element types match this pal
                    if any(element in pal_element_types for element in element_types):
                        matching_character_ids.append(code_name)

                # Add condition to filter by these character_ids
                if matching_character_ids:
                    character_conditions = []
                    for char_id in matching_character_ids:
                        character_conditions.append(UPSPalModel.character_id == char_id)
                    conditions.append(or_(*character_conditions))

            # Filter by pal types (alpha, lucky, human, predator, oilrig, summon)
            if pal_types:
                type_conditions = []
                for pal_type in pal_types:
                    if pal_type == "alpha":
                        # Check for is_boss: true in pal_data
                        type_conditions.append(
                            UPSPalModel.pal_data.like('%"is_boss": true%')
                        )
                    elif pal_type == "lucky":
                        # Check for is_lucky: true in pal_data
                        type_conditions.append(
                            UPSPalModel.pal_data.like('%"is_lucky": true%')
                        )
                    elif pal_type == "human":
                        # Humans are identified by is_pal: false in pals.json
                        pals_json = JsonManager("data/json/pals.json")
                        pals_data = pals_json.read()
                        human_character_ids = [
                            code_name
                            for code_name, pal_info in pals_data.items()
                            if not pal_info.get("is_pal", True)
                        ]
                        if human_character_ids:
                            human_conditions = []
                            for char_id in human_character_ids:
                                human_conditions.append(
                                    UPSPalModel.character_id == char_id
                                )
                            type_conditions.append(or_(*human_conditions))
                    elif pal_type == "predator":
                        # Check character_id contains "predator_"
                        type_conditions.append(
                            UPSPalModel.character_id.like("%predator_%")
                        )
                    elif pal_type == "oilrig":
                        # Check character_id contains "_oilrig"
                        type_conditions.append(
                            UPSPalModel.character_id.like("%_oilrig%")
                        )
                    elif pal_type == "summon":
                        # Check character_id contains "summon_"
                        type_conditions.append(
                            UPSPalModel.character_id.like("%summon_%")
                        )
                if type_conditions:
                    conditions.append(or_(*type_conditions))

            if conditions:
                query = query.where(and_(*conditions))

            # Apply sorting
            sort_column = getattr(UPSPalModel, sort_by, UPSPalModel.created_at)
            if sort_order == "desc":
                query = query.order_by(desc(sort_column))
            else:
                query = query.order_by(asc(sort_column))

            # Get total count
            count_query = select(func.count()).select_from(query.subquery())
            total_count = session.exec(count_query).one()

            # Apply pagination
            query = query.offset(offset).limit(limit)

            pals = session.exec(query).all()

            # Create detached copies with all necessary attributes
            for pal in pals:
                session.expunge(pal)

            return pals, total_count

    @staticmethod
    def get_pal_by_id(pal_id: int) -> Optional[UPSPalModel]:
        with Session(engine) as session:
            pal = session.get(UPSPalModel, pal_id)
            if pal:
                # Create a detached copy with all necessary attributes
                session.expunge(pal)
            return pal

    @staticmethod
    def update_pal(pal_id: int, updates: Dict[str, Any]) -> Optional[UPSPalModel]:
        with Session(engine) as session:
            pal = session.get(UPSPalModel, pal_id)
            if not pal:
                return None

            for key, value in updates.items():
                if hasattr(pal, key):
                    setattr(pal, key, value)

            pal.updated_at = datetime.now(dt.timezone.utc)
            session.commit()
            session.refresh(pal)

            # Update collection counts if collection_id was changed
            if "collection_id" in updates:
                UPSService._update_collection_counts(session)

            # Ensure all attributes are loaded before expunging
            # Access all the attributes that will be needed after expunging
            _ = pal.id
            _ = pal.instance_id
            _ = pal.character_id
            _ = pal.nickname
            _ = pal.level
            _ = pal.collection_id
            _ = pal.tags
            _ = pal.notes
            _ = pal.updated_at

            # Create a detached copy with all necessary attributes
            session.expunge(pal)
            return pal

    @staticmethod
    def delete_pals(pal_ids: List[int]) -> int:
        with Session(engine) as session:
            deleted_count = 0
            for pal_id in pal_ids:
                pal = session.get(UPSPalModel, pal_id)
                if pal:
                    UPSService._log_transfer(
                        session=session,
                        pal_id=pal.id,
                        operation_type="delete",
                        source_type="ups",
                        success=True,
                    )

                    session.delete(pal)
                    deleted_count += 1

            session.commit()

            # Update statistics and collection counts
            UPSService._update_stats(session)
            UPSService._update_collection_counts(session)

            return deleted_count

    @staticmethod
    def create_collection(
        name: str, description: Optional[str] = None, color: Optional[str] = None
    ) -> UPSCollectionModel:
        with Session(engine) as session:
            collection = UPSCollectionModel(
                name=name,
                description=description,
                color=color,
            )
            session.add(collection)
            session.commit()
            session.refresh(collection)

            # Create a detached copy with all necessary attributes
            session.expunge(collection)
            return collection

    @staticmethod
    def get_collections() -> List[UPSCollectionModel]:
        with Session(engine) as session:
            collections = session.exec(
                select(UPSCollectionModel).order_by(UPSCollectionModel.name)
            ).all()

            # Create detached copies with all necessary attributes
            for collection in collections:
                session.expunge(collection)

            return collections

    @staticmethod
    def update_collection(
        collection_id: int, updates: Dict[str, Any]
    ) -> Optional[UPSCollectionModel]:
        with Session(engine) as session:
            collection = session.get(UPSCollectionModel, collection_id)
            if not collection:
                return None

            for key, value in updates.items():
                if hasattr(collection, key):
                    setattr(collection, key, value)

            collection.updated_at = datetime.now(dt.timezone.utc)
            session.commit()
            session.refresh(collection)

            # Create a detached copy with all necessary attributes
            session.expunge(collection)
            return collection

    @staticmethod
    def delete_collection(collection_id: int) -> bool:
        with Session(engine) as session:
            collection = session.get(UPSCollectionModel, collection_id)
            if not collection:
                return False

            pals_query = select(UPSPalModel).where(
                UPSPalModel.collection_id == collection_id
            )
            pals = session.exec(pals_query).all()
            for pal in pals:
                pal.collection_id = None

            session.delete(collection)
            session.commit()
            return True

    @staticmethod
    def get_available_tags() -> List[UPSTagModel]:
        with Session(engine) as session:
            tags = session.exec(select(UPSTagModel).order_by(UPSTagModel.name)).all()

            # Create detached copies with all necessary attributes
            for tag in tags:
                session.expunge(tag)

            return tags

    @staticmethod
    def create_or_update_tag(
        name: str, description: Optional[str] = None, color: Optional[str] = None
    ) -> UPSTagModel:
        with Session(engine) as session:
            existing_tag = session.exec(
                select(UPSTagModel).where(UPSTagModel.name == name)
            ).first()

            if existing_tag:
                if description is not None:
                    existing_tag.description = description
                if color is not None:
                    existing_tag.color = color
                existing_tag.updated_at = datetime.now(dt.timezone.utc)
                session.commit()
                session.refresh(existing_tag)

                # Create a detached copy with all necessary attributes
                session.expunge(existing_tag)
                return existing_tag
            else:
                # Create new
                tag = UPSTagModel(name=name, description=description, color=color)
                session.add(tag)
                session.commit()
                session.refresh(tag)

                # Create a detached copy with all necessary attributes
                session.expunge(tag)
                return tag

    @staticmethod
    def update_tag(tag_id: int, updates: Dict[str, Any]) -> Optional[UPSTagModel]:
        with Session(engine) as session:
            tag = session.get(UPSTagModel, tag_id)
            if not tag:
                return None

            old_name = tag.name

            for key, value in updates.items():
                if hasattr(tag, key):
                    setattr(tag, key, value)

            tag.updated_at = datetime.now(dt.timezone.utc)
            session.commit()
            session.refresh(tag)

            # If name was changed, update all pal tags that reference the old name
            if "name" in updates and old_name != tag.name:
                UPSService._update_pal_tags_on_rename(session, old_name, tag.name)

            # Ensure all attributes are loaded before expunging
            # Access all the attributes that will be needed after expunging
            _ = tag.id
            _ = tag.name
            _ = tag.description
            _ = tag.color
            _ = tag.usage_count
            _ = tag.created_at
            _ = tag.updated_at

            # Create a detached copy with all necessary attributes
            session.expunge(tag)
            return tag

    @staticmethod
    def delete_tag(tag_id: int) -> bool:
        with Session(engine) as session:
            tag = session.get(UPSTagModel, tag_id)
            if not tag:
                return False

            tag_name = tag.name

            # Remove this tag from all pals that have it
            UPSService._remove_tag_from_all_pals(session, tag_name)

            session.delete(tag)
            session.commit()
            return True

    @staticmethod
    def get_stats() -> UPSStatsModel:
        with Session(engine) as session:
            stats = session.get(UPSStatsModel, 1)
            if not stats:
                # Create initial stats
                stats = UPSStatsModel()
                session.add(stats)
                session.commit()
                session.refresh(stats)

            UPSService._update_stats(session)
            session.refresh(stats)

            # Create a detached copy with all necessary attributes
            session.expunge(stats)
            return stats

    @staticmethod
    def export_pal_to_save(
        pal_id: int, destination_type: str, destination_info: Dict[str, Any]
    ) -> bool:
        with Session(engine) as session:
            pal = session.get(UPSPalModel, pal_id)
            if not pal:
                return False

            pal.last_accessed_at = datetime.now(dt.timezone.utc)
            pal.transfer_count += 1
            session.commit()

            UPSService._log_transfer(
                session=session,
                pal_id=pal.id,
                operation_type="export",
                source_type="ups",
                destination_type=destination_type,
                save_file_name=destination_info.get("save_file_name"),
                player_name=destination_info.get("player_name"),
                player_uid=destination_info.get("player_uid"),
                success=True,
            )

            return True

    @staticmethod
    def clone_pal(pal_id: int) -> Optional[UPSPalModel]:
        with Session(engine) as session:
            original_pal = session.get(UPSPalModel, pal_id)
            if not original_pal:
                return None

            clone_pal = UPSPalModel(
                character_id=original_pal.character_id,
                nickname=f"{original_pal.nickname} (Clone)"
                if original_pal.nickname
                else None,
                level=original_pal.level,
                pal_data=original_pal.pal_data.copy(),
                source_save_file=original_pal.source_save_file,
                source_player_uid=original_pal.source_player_uid,
                source_player_name=original_pal.source_player_name,
                source_storage_type="ups_clone",
                collection_id=original_pal.collection_id,
                tags=original_pal.tags.copy(),
                notes=f"Clone of {original_pal.nickname or original_pal.character_id}",
            )

            session.add(clone_pal)

            original_pal.clone_count += 1

            session.commit()
            session.refresh(clone_pal)

            # Update statistics and collection counts
            UPSService._update_stats(session)
            UPSService._update_collection_counts(session)

            UPSService._log_transfer(
                session=session,
                pal_id=clone_pal.id,
                operation_type="clone",
                source_type="ups",
                destination_type="ups",
                success=True,
            )

            # Create a detached copy with all necessary attributes
            session.expunge(clone_pal)
            return clone_pal

    @staticmethod
    def _update_stats(session: Session):
        stats = session.get(UPSStatsModel, 1)
        if not stats:
            stats = UPSStatsModel()
            session.add(stats)

        stats.total_pals = session.exec(select(func.count(UPSPalModel.id))).one()
        stats.total_collections = session.exec(
            select(func.count(UPSCollectionModel.id))
        ).one()
        stats.total_tags = session.exec(select(func.count(UPSTagModel.id))).one()
        stats.total_transfers = (
            session.exec(
                select(func.sum(UPSPalModel.transfer_count)).select_from(UPSPalModel)
            ).one()
            or 0
        )
        stats.total_clones = (
            session.exec(
                select(func.sum(UPSPalModel.clone_count)).select_from(UPSPalModel)
            ).one()
            or 0
        )

        most_transferred = session.exec(
            select(UPSPalModel.id).order_by(desc(UPSPalModel.transfer_count)).limit(1)
        ).first()
        if most_transferred:
            stats.most_transferred_pal_id = most_transferred

        most_cloned = session.exec(
            select(UPSPalModel.id).order_by(desc(UPSPalModel.clone_count)).limit(1)
        ).first()
        if most_cloned:
            stats.most_cloned_pal_id = most_cloned

        most_popular_char = session.exec(
            select(UPSPalModel.character_id)
            .select_from(UPSPalModel)
            .group_by(UPSPalModel.character_id)
            .order_by(desc(func.count(UPSPalModel.character_id)))
            .limit(1)
        ).first()
        if most_popular_char:
            stats.most_popular_character_id = most_popular_char

        # Calculate storage size in MB
        # Get all pal_data and calculate total size
        all_pals = session.exec(select(UPSPalModel.pal_data)).all()
        total_bytes = 0
        for pal_data in all_pals:
            # Convert pal_data dict to JSON string and get byte size
            json_str = json.dumps(pal_data)
            total_bytes += len(json_str.encode("utf-8"))

        stats.storage_size_mb = total_bytes / (1024 * 1024)  # Convert bytes to MB

        # Calculate elemental distribution and special categories
        UPSService._calculate_elemental_and_special_stats(session, stats)

        stats.last_updated = datetime.now(dt.timezone.utc)
        session.commit()

    @staticmethod
    def _calculate_elemental_and_special_stats(session: Session, stats: UPSStatsModel):
        pals_json = JsonManager("data/json/pals.json")
        pals_data = pals_json.read()

        all_pals = session.exec(select(UPSPalModel)).all()
        logger.debug(f"Processing {len(all_pals)} pals for elemental and special stats")

        element_counts = {}
        alpha_count = 0
        lucky_count = 0
        human_count = 0
        predator_count = 0
        oilrig_count = 0
        summon_count = 0

        for pal in all_pals:
            char_id = pal.character_id
            pal_data_dict = pal.pal_data

            # Count element types
            if char_id in pals_data:
                character_data = pals_data[char_id]
                element_types = character_data.get("element_types", [])

                for element_type in element_types:
                    element_counts[element_type] = (
                        element_counts.get(element_type, 0) + 1
                    )

                if not character_data.get("is_pal", True):
                    human_count += 1

            # Count special categories from pal_data
            if pal_data_dict:
                if pal_data_dict.get("is_boss", False):
                    alpha_count += 1

                if pal_data_dict.get("is_lucky", False):
                    lucky_count += 1

            char_id_lower = char_id.lower()
            if "predator_" in char_id_lower:
                predator_count += 1
            elif "_oilrig" in char_id_lower:
                oilrig_count += 1
            elif "summon_" in char_id_lower:
                summon_count += 1

        stats.element_distribution = json.dumps(element_counts)
        stats.alpha_count = alpha_count
        stats.lucky_count = lucky_count
        stats.human_count = human_count
        stats.predator_count = predator_count
        stats.oilrig_count = oilrig_count
        stats.summon_count = summon_count

    @staticmethod
    def _log_transfer(
        session: Session,
        pal_id: int,
        operation_type: str,
        source_type: Optional[str] = None,
        destination_type: Optional[str] = None,
        source_location: Optional[str] = None,
        destination_location: Optional[str] = None,
        save_file_name: Optional[str] = None,
        player_name: Optional[str] = None,
        player_uid: Optional[UUID] = None,
        success: bool = True,
        error_message: Optional[str] = None,
    ):
        log_entry = UPSTransferLogModel(
            pal_id=pal_id,
            operation_type=operation_type,
            source_type=source_type,
            destination_type=destination_type,
            source_location=source_location,
            destination_location=destination_location,
            save_file_name=save_file_name,
            player_name=player_name,
            player_uid=player_uid,
            success=success,
            error_message=error_message,
        )
        session.add(log_entry)

    @staticmethod
    def nuke_all_pals() -> int:
        """Delete ALL pals from UPS storage.

        Returns:
            int: Number of pals deleted
        """
        with Session(engine) as session:
            # Get count before deletion for logging
            total_count = session.exec(select(func.count(UPSPalModel.id))).one()

            if total_count == 0:
                return 0

            # Log the nuke operation before deletion
            logger.warning(
                f"NUKE OPERATION: Deleting ALL {total_count} pals from UPS storage"
            )

            # Get all pal IDs for logging individual deletions
            all_pal_ids = session.exec(select(UPSPalModel.id)).all()

            # Log individual deletions
            for pal_id in all_pal_ids:
                UPSService._log_transfer(
                    session=session,
                    pal_id=pal_id,
                    operation_type="nuke_delete",
                    source_type="ups",
                    success=True,
                )

            # Delete all pals in batches for better performance
            deleted_count = 0
            batch_size = 100

            while True:
                # Get a batch of pals to delete
                batch_query = select(UPSPalModel).limit(batch_size)
                batch_pals = session.exec(batch_query).all()

                if not batch_pals:
                    break

                # Delete this batch
                for pal in batch_pals:
                    session.delete(pal)
                    deleted_count += 1

                # Commit this batch
                session.commit()

            # Reset all collection pal_counts to 0
            collections = session.exec(select(UPSCollectionModel)).all()
            for collection in collections:
                collection.pal_count = 0
                collection.updated_at = datetime.utcnow()

            # Update global statistics
            UPSService._update_stats(session)

            session.commit()

            logger.warning(
                f"NUKE OPERATION COMPLETED: Deleted {deleted_count} pals from UPS storage"
            )
            return deleted_count

    @staticmethod
    def _update_pal_tags_on_rename(session: Session, old_name: str, new_name: str):
        """Update all pal tags when a tag is renamed."""
        pals_with_tag = session.exec(
            select(UPSPalModel).where(
                UPSPalModel.tags.like(f"%{json.dumps(old_name)}%")
            )
        ).all()

        for pal in pals_with_tag:
            if old_name in pal.tags:
                # Replace old tag name with new tag name
                pal.tags = [new_name if tag == old_name else tag for tag in pal.tags]
                pal.updated_at = datetime.now(dt.timezone.utc)

        session.commit()

    @staticmethod
    def _remove_tag_from_all_pals(session: Session, tag_name: str):
        """Remove a specific tag from all pals that have it."""
        pals_with_tag = session.exec(
            select(UPSPalModel).where(
                UPSPalModel.tags.like(f"%{json.dumps(tag_name)}%")
            )
        ).all()

        for pal in pals_with_tag:
            if tag_name in pal.tags:
                # Remove the tag from the list
                pal.tags = [tag for tag in pal.tags if tag != tag_name]
                pal.updated_at = datetime.now(dt.timezone.utc)

        session.commit()

    @staticmethod
    def _update_collection_counts(session: Session):
        """Update pal_count for all collections."""
        collections = session.exec(select(UPSCollectionModel)).all()

        for collection in collections:
            # Count pals in this collection
            pal_count = session.exec(
                select(func.count(UPSPalModel.id)).where(
                    UPSPalModel.collection_id == collection.id
                )
            ).one()

            # Update the collection's pal_count
            collection.pal_count = pal_count
            collection.updated_at = datetime.utcnow()

        session.commit()
