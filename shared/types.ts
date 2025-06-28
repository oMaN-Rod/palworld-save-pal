/* tslint:disable */
/* eslint-disable */
/**
/* This file was automatically generated from pydantic models by running pydantic2ts.
/* Do not modify it by hand - just update the pydantic models and then re-run the script
*/

export type PalGender = "Male" | "Female";
export type ElementType = "Fire" | "Water" | "Ground" | "Ice" | "Neutral" | "Dark" | "Grass" | "Dragon" | "Electric";
export type EntryState = "None" | "Modified" | "New" | "Deleted";
export type CharacterContainerType = "PalBox" | "Party" | "Base";
export type SaveFileType = "gamepass" | "steam";

export interface Base {
  id: string;
  name?: string | null;
  pals: {
    [k: string]: Pal;
  };
  container_id: string;
  pal_container: CharacterContainer;
  slot_count: number;
  storage_containers: {
    [k: string]: ItemContainer;
  };
  state: EntryState;
  location: WorldMapPoint;
}
export interface Pal {
  name: string;
  instance_id: string;
  owner_uid: string;
  character_id: string;
  character_key: string;
  is_lucky: boolean;
  is_boss: boolean;
  is_predator: boolean;
  gender: PalGender;
  rank_hp: number;
  rank_attack: number;
  rank_defense: number;
  rank_craftspeed: number;
  talent_hp: number;
  talent_shot: number;
  talent_defense: number;
  rank: number;
  level: number;
  nickname?: string | null;
  is_tower: boolean;
  stomach: number;
  storage_id?: string | null;
  storage_slot: number;
  learned_skills: string[];
  active_skills: string[];
  passive_skills: string[];
  work_suitability: {
    [k: string]: number;
  };
  hp: number;
  max_hp: number;
  elements: ElementType[];
  state: EntryState;
  sanity: number;
  exp: number;
  is_sick: boolean;
}
export interface CharacterContainer {
  id: string;
  player_uid: string;
  type: CharacterContainerType;
  size?: number | null;
  slots?: CharacterContainerSlot[] | null;
}
export interface CharacterContainerSlot {
  slot_index: number;
  pal_id?: string | null;
}
export interface ItemContainer {
  id: string;
  type: string;
  slots: ItemContainerSlot[];
  key: string;
  slot_num: number;
  state?: EntryState | null;
}
export interface ItemContainerSlot {
  slot_index: number;
  static_id: string;
  count: number;
  dynamic_item?: DynamicItem | null;
}
export interface DynamicItem {
  local_id: string;
  durability: number;
  remaining_bullets?: number | null;
  type: string;
  character_id?: string | null;
  character_key?: string | null;
  gender: string;
  talent_hp: number;
  talent_shot: number;
  talent_defense: number;
  learned_skills: string[];
  active_skills: string[];
  passive_skills: string[];
  modified: boolean;
}
export interface WorldMapPoint {
  x: number;
  y: number;
  z: number;
}
export interface BaseDTO {
  id: string;
  storage_containers: {
    [k: string]: ItemContainer;
  };
}
export interface BaseMessage {
  type: string;
  data?: {
    [k: string]: unknown;
  };
}
export interface EggConfig {
  character_id: string;
  gender: PalGender;
  talent_hp: number;
  talent_shot: number;
  talent_defense: number;
  learned_skills: string[];
  active_skills: string[];
  passive_skills: string[];
}
export interface ExStatusPointList {
  max_hp: number;
  max_sp: number;
  attack: number;
  weight: number;
  work_speed: number;
}
export interface GamePassContainer {
  path: string;
  guid: string;
  num: number;
  name: string;
}
export interface GamepassSave {
  save_id: string;
  world_name: string;
  player_count: number;
  containers: GamePassContainer[];
}
export interface GetWorkSuitabilityMessage {
  type?: string;
  data?: {
    [k: string]: unknown;
  };
}
export interface Guild {
  admin_player_uid: string;
  bases: {
    [k: string]: Base;
  };
  id: string;
  name: string;
  players: string[];
  container_id?: string | null;
  guild_chest?: ItemContainer | null;
  lab_research_data?: GuildLabResearchInfo[] | null;
  state: EntryState;
}
export interface GuildLabResearchInfo {
  research_id: string;
  work_amount: number;
}
export interface GuildDTO {
  name?: string | null;
  base?: BaseDTO | null;
  guild_chest?: ItemContainer | null;
  lab_research?: GuildLabResearchInfo[] | null;
}
export interface MapObject {
  x: number;
  y: number;
  z: number;
  type: string;
  localized_name: string;
  pal: string;
}
export interface Message {
  type: string;
  data?: {
    [k: string]: unknown;
  };
}
export interface Player {
  uid: string;
  nickname: string;
  level: number;
  hp: number;
  pals?: {
    [k: string]: Pal;
  } | null;
  dps?: {
    [k: string]: Pal;
  } | null;
  pal_box_id: string;
  pal_box: CharacterContainer;
  otomo_container_id: string;
  party: CharacterContainer;
  common_container: ItemContainer;
  essential_container: ItemContainer;
  weapon_load_out_container: ItemContainer;
  player_equipment_armor_container: ItemContainer;
  food_equip_container: ItemContainer;
  state: EntryState;
  exp: number;
  stomach: number;
  sanity: number;
  status_point_list: StatusPointList;
  ex_status_point_list: ExStatusPointList;
  guild_id: string;
  technologies: string[];
  technology_points: number;
  boss_technology_points: number;
  location: WorldMapPoint;
  last_online_time: string;
}
export interface StatusPointList {
  max_hp: number;
  max_sp: number;
  attack: number;
  weight: number;
  capture_rate: number;
  work_speed: number;
}
export interface SaveFile {
  name: string;
  type: SaveFileType;
  world_name?: string | null;
  size?: number | null;
}
export interface SettingsDTO {
  language: string;
  clone_prefix: string;
  new_pal_prefix: string;
  debug_mode: boolean;
  cheat_mode: boolean;
}
export interface UpdateSettingsMessage {
  type?: string;
  data: SettingsDTO;
}
