// Backend types
export type Pal = {
	name: string;
	instance_id: string;
	owner_uid: string;
	is_lucky: boolean;
	is_boss: boolean;
	character_id: string;
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
	nickname?: string;
	is_tower: boolean;
	stomach: number;
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
	exp: number;
};

export type Player = {
	uid: string;
	nickname: string;
	level: number;
	pals?: Record<string, Pal>;
	pal_box_id: string;
	otomo_container_id: string;
	common_container: ItemContainer;
	essential_container: ItemContainer;
	weapon_load_out_container: ItemContainer;
	player_equipment_armor_container: ItemContainer;
	food_equip_container: ItemContainer;
	state: EntryState;
	exp: number;
};

export type SaveFile = {
	name: string;
	size: number;
};
export interface DynamicItem {
	local_id: string;
	durability: number;
	remaining_bullets?: number;
	type: DynamicItemClass;
}

export enum ItemTypeA {
	None,
	Weapon,
	SpecialWeapon,
	Armor,
	Accessory,
	Material,
	Consume,
	Ammo,
	Food,
	Essential,
	Glider,
	MonsterEquipWeapon,
	Blueprint
}

export enum ItemTypeB {
	None,
	WeaponMelee,
	WeaponBow,
	WeaponCrossbow,
	WeaponHandgun,
	WeaponAssaultRifle,
	WeaponSniperRifle,
	WeaponRocketLauncher,
	WeaponShotgun,
	WeaponFlameThrower,
	WeaponGatlingGun,
	WeaponCollectionTool,
	WeaponThrowObject,
	WeaponGrapplingGun,
	SPWeaponCaptureBall,
	SPWeaponDamageTrap,
	SPWeaponCaptureTrap,
	SPWeaponCaptureRope,
	ArmorHead,
	ArmorBody,
	Accessory,
	MaterialOre,
	MaterialJewelry,
	MaterialIngot,
	MaterialWood,
	MaterialStone,
	MaterialProccessing,
	MaterialMonster,
	MaterialPalEgg,
	ConsumeBandage,
	ConsumeSeed,
	ConsumeBullet,
	ConsumeWazaMachine,
	ConsumeTechnologyBook,
	ConsumeAncientTechnologyBook,
	ConsumeOther,
	ConsumeGainStatusPoints,
	ConsumePalLevelUp,
	ConsumePalGainExp,
	ConsumePalTalentUp,
	ConsumePalRankUp,
	FoodMeat,
	FoodVegetable,
	FoodFish,
	FoodDishMeat,
	FoodDishVegetable,
	FoodDishFish,
	FoodProcessed,
	Essential,
	Essential_UnlockPlayerFuture,
	Glider,
	Shield,
	Money,
	Medicine,
	Drug,
	MonsterEquipWeapon,
	Blueprint,
	ReturnToBaseCamp,
	Essential_PalGear
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

export enum Rarity {
	Common,
	Uncommon,
	Rare,
	Epic,
	Legendary
}
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
export type DynamicItemClass = 'armor' | 'weapon';
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
	durability: number;
	magazine_size?: number;
	type: DynamicItemClass;
}
export interface ItemDetails {
	group: ItemGroup;
	weight: number;
	type_a: ItemTypeA;
	type_b: ItemTypeB;
	price: number;
	icon: string;
	rank: number;
	rarity: Rarity;
	max_stack_count: number;
	sort_id: number;
	damage?: number;
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

export enum EPalElementType {
	None = 'EPalElementType::None',
	Normal = 'EPalElementType::Normal',
	Fire = 'EPalElementType::Fire',
	Water = 'EPalElementType::Water',
	Leaf = 'EPalElementType::Leaf',
	Electricity = 'EPalElementType::Electricity',
	Ice = 'EPalElementType::Ice',
	Earth = 'EPalElementType::Earth',
	Dark = 'EPalElementType::Dark',
	Dragon = 'EPalElementType::Dragon',
	MAX = 'EPalElementType::MAX'
}

export type ElementType =
	| 'Fire'
	| 'Water'
	| 'Ground'
	| 'Ice'
	| 'Neutral'
	| 'Dark'
	| 'Grass'
	| 'Dragon'
	| 'Electric';

export function mapElementType(
	type: EPalElementType | ElementType
): ElementType | EPalElementType | null {
	const mapping: Record<string, string> = {
		[EPalElementType.None]: 'Neutral',
		[EPalElementType.Normal]: 'Neutral',
		[EPalElementType.Fire]: 'Fire',
		[EPalElementType.Water]: 'Water',
		[EPalElementType.Leaf]: 'Grass',
		[EPalElementType.Electricity]: 'Electric',
		[EPalElementType.Ice]: 'Ice',
		[EPalElementType.Earth]: 'Ground',
		[EPalElementType.Dark]: 'Dark',
		[EPalElementType.Dragon]: 'Dragon',
		[EPalElementType.MAX]: 'Neutral',
		Fire: EPalElementType.Fire,
		Water: EPalElementType.Water,
		Ground: EPalElementType.Earth,
		Ice: EPalElementType.Ice,
		Neutral: EPalElementType.Normal,
		Dark: EPalElementType.Dark,
		Grass: EPalElementType.Leaf,
		Dragon: EPalElementType.Dragon,
		Electric: EPalElementType.Electricity
	};

	const result = mapping[type];

	if (result === undefined) {
		console.warn(`No mapping found for element type: ${type}`);
		return null;
	}

	return result as ElementType | EPalElementType;
}
export interface PalData {
	localized_name: string;
	description: string;
	is_pal: boolean;
	tribe: string;
	pal_deck_index: number;
	size: string;
	rarity: number;
	element_types: EPalElementType[];
	genus_category: string;
	organization: string;
	weapon: string;
	weapon_equip: boolean;
	scaling: Scaling;
	enemy_max_hp_rate: number;
	enemy_receive_damage_rate: number;
	enemy_inflict_damage_rate: number;
	capture_rate_correct: number;
	exp_ratio: number;
	price: number;
	slow_walk_speed: number;
	walk_speed: number;
	run_speed: number;
	ride_sprint_speed: number;
	transport_speed: number;
	is_boss: boolean;
	is_tower_boss: boolean;
	is_raid_boss: boolean;
	max_full_stomach: number;
	food_amount: number;
	nocturnal: boolean;
	biological_grade: number;
	predator: boolean;
	edible: boolean;
	stamina: number;
	male_probability: number;
	combi_rank: number;
	work_suitability: Record<WorkSuitability, number>;
	passive_skills: string[];
	skill_set?: Record<string, number>;
}

export interface Bonuses {
	attack: number;
	defense: number;
	work_speed: number;
}

export enum PalGender {
	MALE = 'Male',
	FEMALE = 'Female'
}

export type SkillType = 'Active' | 'Passive' | 'Empty';

export interface ActiveSkillDetails {
	type: string;
	power: number;
	ct: number;
	name: string;

	exclusive?: string[];
}

export interface Skill {
	id: string;
	name: string;
	description: string;
}

export interface ActiveSkill extends Skill {
	details: ActiveSkillDetails;
}

export interface PassiveSkillDetails {
	tier: string;
	bonuses: Bonuses;
}

export interface PassiveSkill extends Skill {
	details: PassiveSkillDetails;
}

export type WorkSuitability =
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

export type Element = {
	name: string;
	color: string;
	icon: string;
	badge_icon: string;
	egg_icon: string;
	fruit_icon: string;
	white_icon: string;
};

export enum EntryState {
	NONE = 'None',
	MODIFIED = 'Modified',
	NEW = 'New',
	DELETED = 'Deleted'
}
