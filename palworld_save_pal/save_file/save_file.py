import copy
import json
import os
from typing import Any, Dict, List, Optional
from uuid import UUID
from pydantic import BaseModel, ConfigDict, PrivateAttr

from palworld_save_tools.gvas import GvasFile
from palworld_save_tools.json_tools import CustomEncoder
from palworld_save_tools.palsav import compress_gvas_to_sav, decompress_sav_to_gvas
from palworld_save_tools.paltypes import (
    DISABLED_PROPERTIES,
    PALWORLD_CUSTOM_PROPERTIES,
    PALWORLD_TYPE_HINTS,
)

from palworld_save_pal.pals.pal import Pal
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.pals.models import PalSummary, Player
from palworld_save_pal.save_file.utils import safe_get, safe_set
from palworld_save_pal.save_file.empty_objects import (
    EMPTY_ENUM_ARRAY_PROPERTY,
    EMPTY_STR_PROPERTY,
)

logger = create_logger(__name__)

CUSTOM_PROPERTIES = {
    k: v for k, v in PALWORLD_CUSTOM_PROPERTIES.items() if k not in DISABLED_PROPERTIES
}


class SaveFile(BaseModel):
    name: str = ""
    size: int = 0
    gvas_file: Optional[GvasFile] = None

    _pals: Dict[UUID, PalSummary] = PrivateAttr(default_factory=dict)
    _character_save_parameter_map: List[Dict[str, Any]] = PrivateAttr(
        default_factory=dict
    )
    _character_container_save_data: List[Dict[str, Any]] = PrivateAttr(
        default_factory=dict
    )

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def _get_file_size(self, data: bytes):
        if hasattr(data, "seek") and hasattr(data, "tell"):
            data.seek(0, os.SEEK_END)
            self.size = data.tell()
            data.seek(0)
        else:
            self.size = data.__sizeof__()

    def _get_player_pals(self, uid):
        pals = {}
        pals = {k: v for k, v in self._pals.items() if f"{v.owner_uid}" == uid}
        logger.debug("Loaded %d pals for player %s", len(pals), uid)
        return pals

    def _get_world_save_data(self, deep_copy=True):
        world_save_data = safe_get(self.gvas_file.properties, "worldSaveData", "value")
        if deep_copy:
            return copy.deepcopy(world_save_data)
        return world_save_data

    def _is_player(self, entry):
        is_player_path = [
            "value",
            "RawData",
            "value",
            "object",
            "SaveParameter",
            "value",
            "IsPlayer",
            "value",
        ]
        return safe_get(entry, *is_player_path, default=False)

    def _load_pals(self):
        if not self.gvas_file:
            raise ValueError("No GvasFile has been loaded.")
        logger.debug("Loading pals")
        self._pals = {}
        for e in self._character_save_parameter_map:
            if self._is_player(e):
                continue
            instance = Pal.create_summary(e)
            if instance:
                pal_summary = PalSummary(**instance.model_dump())
                self._pals[instance.instance_id] = pal_summary
            else:
                logger.warning("Failed to create PalEntity summary")
        logger.debug("Total pals loaded: %d", len(self._pals))

    def _set_active_data(self) -> None:
        world_save_data = self._get_world_save_data()
        self._character_save_parameter_map = safe_get(
            world_save_data, "CharacterSaveParameterMap", "value", default=[]
        )
        self._character_container_save_data = safe_get(
            world_save_data, "CharacterContainerSaveData", "value", default=[]
        )

    def get_pal(self, instance_id: UUID):
        for e in self._character_save_parameter_map:
            if (
                not self._is_player(e)
                and str(e["key"]["InstanceId"]["value"]).lower()
                == str(instance_id).lower()
            ):
                return Pal.create_safe(copy.deepcopy(e))
        return None

    def get_pals(self):
        return self._pals

    def get_players(self):
        if not self._character_save_parameter_map:
            return {}

        def extract_player_info(entry):
            uid = safe_get(entry, "key", "PlayerUId", "value")
            nickname_path = [
                "value",
                "RawData",
                "value",
                "object",
                "SaveParameter",
                "value",
                "NickName",
                "value",
            ]
            nickname = safe_get(entry, *nickname_path)
            level_path = [
                "value",
                "RawData",
                "value",
                "object",
                "SaveParameter",
                "value",
                "Level",
                "value",
            ]
            level = safe_get(entry, *level_path)
            player = Player(uid=uid.UUID(), nickname=nickname, level=level)
            player.pals = self._get_player_pals(uid)
            return player

        players = {
            x.uid: x
            for x in [
                extract_player_info(x)
                for x in self._character_save_parameter_map
                if self._is_player(x)
            ]
        }
        return players

    def load_gvas(self, data: bytes):
        logger.info("Loading %s as GVAS", self.name)
        raw_gvas, _ = decompress_sav_to_gvas(data)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        self.gvas_file = gvas_file
        self._get_file_size(data)
        self._set_active_data()
        self._load_pals()
        return self

    def load_json(self, data: bytes):
        logger.info("Loading %s as JSON", self.name)
        self.gvas_file = GvasFile.load(json.loads(data))
        return self

    def pal_count(self):
        return len(self._pals)

    def get_json(self, minify=False, allow_nan=True):
        logger.info("Converting %s to JSON", self.name)
        return json.dumps(
            self.gvas_file.dump(),
            indent=None if minify else "\t",
            cls=CustomEncoder,
            allow_nan=allow_nan,
        )

    def sav(self):
        logger.info("Converting %s to SAV", self.name)
        if (
            "Pal.PalWorldSaveGame" in self.gvas_file.header.save_game_class_name
            or "Pal.PalLocalWorldSaveGame" in self.gvas_file.header.save_game_class_name
        ):
            save_type = 0x32
        else:
            save_type = 0x31
        return compress_gvas_to_sav(
            self.gvas_file.write(PALWORLD_CUSTOM_PROPERTIES), save_type
        )

    def to_json(
        self,
        output_path,
        minify=False,
        allow_nan=True,
    ):
        logger.info("Converting %s to JSON, saving to %s", self.name, output_path)
        with open(output_path, "w", encoding="utf8") as f:
            indent = None if minify else "\t"
            json.dump(
                self.gvas_file.dump(),
                f,
                indent=indent,
                cls=CustomEncoder,
                allow_nan=allow_nan,
            )

    def to_sav(self, output_path):
        logger.info("Converting %s to SAV, saving to %s", self.name, output_path)
        if (
            "Pal.PalWorldSaveGame" in self.gvas_file.header.save_game_class_name
            or "Pal.PalLocalWorldSaveGame" in self.gvas_file.header.save_game_class_name
        ):
            save_type = 0x32
        else:
            save_type = 0x31
        logger.info("Compressing GVAS to SAV with save type %s", save_type)
        sav_file = compress_gvas_to_sav(
            self.gvas_file.write(PALWORLD_CUSTOM_PROPERTIES), save_type
        )
        with open(output_path, "wb") as f:
            f.write(sav_file)

    async def update_pals(
        self, modified_pals: Dict[UUID, Pal], ws_callback=None
    ) -> None:
        if not self.gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for pal_id, pal in modified_pals.items():
            await ws_callback(f"Updating pal {pal.nickname}")
            self._update_pal(pal_id, pal)
        logger.info("Updated %d pals in the save file.", len(modified_pals))
        await ws_callback("Saving changes to file")
        self._set_active_data()

    def _update_pal(self, pal_id: UUID, pal: Pal) -> None:
        world_save_data = self._get_world_save_data(False)
        character_save_parameter_map = safe_get(
            world_save_data, "CharacterSaveParameterMap", "value", default=[]
        )
        for entry in character_save_parameter_map:
            if (
                str(safe_get(entry, "key", "InstanceId", "value")).lower()
                == str(pal_id).lower()
            ):
                self._update_pal_entry(entry, pal)
                return
        logger.warning("Pal with ID %s not found in the save file.", pal_id)

    def _update_pal_entry(self, entry: Dict[str, Any], pal: Pal) -> None:

        pal_obj = safe_get(
            entry, "value", "RawData", "value", "object", "SaveParameter", "value"
        )
        if not pal_obj:
            logger.error("Invalid pal entry structure for pal %s", pal.instance_id)
            return

        self._update_pal_nickname(pal_obj, pal.nickname)
        self._update_pal_gender(pal_obj, pal.gender)
        self._update_pal_equip_waza(pal_obj, pal.active_skills)
        self._update_mastered_waza(pal_obj, pal.learned_skills)
        self._update_pal_array(pal_obj, "PassiveSkillList", pal.passive_skills)

    def _update_pal_equip_waza(
        self, pal_obj: Dict[str, Any], active_skills: List[str]
    ) -> None:
        if not active_skills or len(active_skills) == 0:
            return
        active_skills = [f"EPalWazaID::{skill}" for skill in active_skills]
        if "EquipWaza" in pal_obj:
            safe_set(pal_obj["EquipWaza"], "value", "values", value=active_skills)
        else:
            pal_obj["EquipWaza"] = EMPTY_ENUM_ARRAY_PROPERTY
            safe_set(pal_obj["EquipWaza"], "value", "values", value=active_skills)

    def _update_pal_gender(self, pal_obj: Dict[str, Any], gender: str) -> None:
        gender = f"EPalGenderType::{gender.capitalize()}"
        safe_set(pal_obj["Gender"], "value", "value", value=gender)

    def _update_pal_nickname(self, pal_obj: Dict[str, Any], nickname: str) -> None:
        if not nickname or len(nickname) == 0:
            return
        if "NickName" in pal_obj:
            safe_set(pal_obj["NickName"], "value", value=nickname)
        else:
            pal_obj["NickName"] = EMPTY_STR_PROPERTY
            safe_set(pal_obj["NickName"], "value", value=nickname)

    def _update_mastered_waza(
        self, pal_obj: Dict[str, Any], learned_skills: List[str]
    ) -> None:
        if not learned_skills or len(learned_skills) == 0:
            return
        if "MasteredWaza" in pal_obj:
            safe_set(pal_obj["MasteredWaza"], "value", "values", value=learned_skills)
        else:
            pal_obj["MasteredWaza"] = EMPTY_ENUM_ARRAY_PROPERTY
            safe_set(pal_obj["MasteredWaza"], "value", "values", value=learned_skills)

    def _update_pal_field(
        self, pal_obj: Dict[str, Any], field: str, value: Any
    ) -> None:
        if field in pal_obj:
            safe_set(pal_obj[field], "value", value=value)
        else:
            logger.warning("Field %s not found in pal object.", field)

    def _update_pal_array(
        self, pal_obj: Dict[str, Any], field: str, values: List[str]
    ) -> None:
        if field in pal_obj:
            safe_set(pal_obj[field], "value", "values", value=values)
        else:
            logger.warning("Array field %s not found in pal object.", field)
