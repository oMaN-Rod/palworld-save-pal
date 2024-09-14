// Backend types
type Pal = {
	name: string;
	instance_id: string;
	owner_uid: string;
	is_lucky: boolean;
	is_boss: boolean;
	character_id: string;
	gender: PalGender;
	work_speed: number;
	rank_hp: number;
	rank_attack: number;
	rank_defense: number;
	rank_craftspeed: number;
	talent_hp: number;
	talent_melee: number;
	talent_shot: number;
	talent_defense: number;
	rank: number;
	level: number;
	nickname?: string;
	is_tower: boolean;
	stomach: number;
	max_stomach: number;
	storage_id?: string;
	storage_slot: number;
	learned_skills: string[];
	active_skills: string[];
	passive_skills: string[];
	work_suitabilities: Record<WorkSuitability, number>;
	hp: number;
	max_hp: number;
	elements: ElementType[];
	state: EntryState;
	sanity: number;
};

type Player = {
	uid: string;
	nickname: string;
	level: number;
	pals?: Record<string, Pal>;
	common_container: ItemContainer;
	essential_container: ItemContainer;
	weapon_load_out_container: ItemContainer;
	player_equipment_armor_container: ItemContainer;
	food_equip_container: ItemContainer;
	state: EntryState;
};

type SaveFile = {
	name: string;
	size: number;
};
export interface DynamicItem {
	local_id: string;
	durability: number;
	remaining_bullets?: number;
	type: DynamicItemType;
}

export interface ItemContainerSlot {
	slot_index: number;
	static_id: string;
	count: number;
	dynamic_item?: DynamicItem;
}

export interface ItemContainer {
	id: string;
	type: string;
	slots: ItemContainerSlot[];
}

export interface PresetProfile {
	name: string;
	common_container?: ItemContainerSlot[];
	essential_container?: ItemContainerSlot[];
	weapon_load_out_container?: ItemContainerSlot[];
	player_equipment_armor_container?: ItemContainerSlot[];
	food_equip_container?: ItemContainerSlot[];
}

// Frontend types

export type Tier = 'None' | 'Uncommon' | 'Rare' | 'Epic' | 'Legendary';
export type ItemType =
	| 'Accessory'
	| 'Ammo'
	| 'Armor'
	| 'Consumable'
	| 'Currency'
	| 'Egg'
	| 'Ingredient'
	| 'Key_Item'
	| 'Material'
	| 'None'
	| 'Pal_Sphere'
	| 'Schematic'
	| 'Structure'
	| 'Unknown'
	| 'Utility'
	| 'Weapon';
export type DynamicItemType = 'armor' | 'weapon';
export type ItemGroup =
	| 'Accessory'
	| 'Body'
	| 'Common'
	| 'Food'
	| 'Glider'
	| 'Head'
	| 'Shield'
	| 'Weapon'
	| 'KeyItem';

export interface DynamicItemDetails {
	damage: number;
	durability: number;
	magazine_size: number;
	workload: number;
	type: DynamicItemType;
}
export interface ItemDetails {
	image: string;
	type: ItemType;
	group: ItemGroup;
	tier: Tier;
	stack: number;
	weight: number;
	buy_price: number;
	sell_price: number;
	dynamic?: DynamicItemDetails;
}

export interface ItemInfo {
	localized_name: string;
	description: string;
}

export interface Item {
	id: string;
	details: ItemDetails;
	info: ItemInfo;
}

export type Scaling = {
	hp: number;
	attack: number;
	defense: number;
};

export type Suitabilities = {
	[key: string]: number;
};

export interface PalData {
	code_name: string;
	localized_name: string;
	type: ElementType[];
	skill_set: string[];
	raid_skill_set?: string[];
	scaling: Scaling;
	suitabilities: Suitabilities;
	tower?: boolean;
	human?: boolean;
	bonuses?: Bonuses;
}

export interface Bonuses {
	attack: number;
	defense: number;
	work_speed: number;
}

enum PalGender {
	MALE = 'Male',
	FEMALE = 'Female'
}

type SkillType = 'Active' | 'Passive' | 'Empty';

interface ActiveSkillDetails {
	type: string;
	power: number;
	ct: number;
	name: string;

	exclusive?: string[];
}

interface Skill {
	id: string;
	name: string;
	description: string;
}

interface ActiveSkill extends Skill {
	details: ActiveSkillDetails;
}

interface PassiveSkillDetails {
	tier: string;
	bonuses: Bonuses;
}

interface PassiveSkill extends Skill {
	details: PassiveSkillDetails;
}

type ElementType =
	| 'Fire'
	| 'Water'
	| 'Ground'
	| 'Ice'
	| 'Neutral'
	| 'Dark'
	| 'Grass'
	| 'Dragon'
	| 'Electric';

type WorkSuitability =
	| 'EmitFlame'
	| 'Watering'
	| 'Seeding'
	| 'GenerateElectricity'
	| 'Handcraft'
	| 'Collection'
	| 'Deforest'
	| 'Mining'
	| 'OilExtraction'
	| 'ProductMedicine'
	| 'Cool'
	| 'Transport'
	| 'MonsterFarm';

type Element = {
	name: string;
	color: string;
	icon: string;
	badge_icon: string;
	egg_icon: string;
	fruit_icon: string;
	white_icon: string;
};

enum EntryState {
	NONE = 'None',
	MODIFIED = 'Modified',
	NEW = 'New',
	DELETED = 'Deleted'
}

export { EntryState, PalGender };
export type {
	ActiveSkill,
	ActiveSkillDetails,
	Element,
	ElementType,
	Pal,
	PassiveSkill,
	PassiveSkillDetails,
	Player,
	SaveFile,
	Skill,
	SkillType,
	WorkSuitability
};
