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

export type EggConfig = {
	character_id: string;
	gender: PalGender;
	talent_hp: number;
	talent_shot: number;
	talent_defense: number;
	learned_skills: string[];
	active_skills: string[];
	passive_skills: string[];
};

export type Pal = {
	name: string;
	instance_id: string;
	owner_uid: string;
	character_id: string;
	character_key: string;
	is_lucky: boolean;
	is_boss: boolean;
	is_predator: boolean;
	friendship_point: number;
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
	work_suitability: Record<WorkSuitability, number>;
	hp: number;
	max_hp: number;
	elements: ElementType[];
	state: EntryState;
	sanity: number;
	exp: number;
	is_sick: boolean;
};

export type StatusPointList = {
	max_hp: number;
	max_sp: number;
	attack: number;
	weight: number;
	capture_rate: number;
	work_speed: number;
};

export type ExStatusPointList = {
	max_hp: number;
	max_sp: number;
	attack: number;
	weight: number;
	work_speed: number;
};

export type CharacterContainerSlot = { slot_index: number; pal_id?: string };

export enum CharacterContainerType {
	PAL_BOX = 'PalBox',
	PARTY = 'Party',
	BASE = 'Base'
}

export type CharacterContainer = {
	id: string;
	player_uid: string;
	type: CharacterContainerType;
	size?: number;
	slots?: CharacterContainerSlot[];
};

export type WorldMapPoint = {
	x: number;
	y: number;
	z: number;
};

export type Player = {
	uid: string;
	nickname: string;
	level: number;
	hp: number;
	pals?: Record<string, Pal>;
	dps?: Record<number, Pal>;
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
};

export type GuildLabResearchInfo = {
	research_id: string;
	work_amount: number;
};

export type Guild = {
	admin_player_uid: string;
	bases: Record<string, Base>;
	id: string;
	name: string;
	players: string[];
	container_id?: string;
	guild_chest?: ItemContainer;
	lab_research_data?: GuildLabResearchInfo[];
	state: EntryState;
};

export type Base = {
	id: string;
	name?: string;
	pals: Record<string, Pal>;
	container_id: string;
	pal_container: CharacterContainer;
	slot_count: number;
	storage_containers: Record<string, ItemContainer>;
	state: EntryState;
	location: WorldMapPoint;
};

export type MapObject = {
	x: number;
	y: number;
	type: string;
	localized_name: string;
	pal: string;
};

export type BaseDTO = { id: string; storage_containers: Record<string, ItemContainer> };

export type GuildDTO = {
	name?: string;
	bases?: Record<string, BaseDTO>;
	guild_chest?: ItemContainer;
	lab_research?: GuildLabResearchInfo[];
};

export type SaveFileType = 'gamepass' | 'steam';

export type SaveFile = { name: string; type: SaveFileType; world_name?: string; size?: number };
export interface DynamicItem {
	local_id: string;
	durability: number;
	remaining_bullets?: number;
	type: DynamicItemClass;
	character_id?: string;
	character_key?: string;
	gender: string;
	talent_hp: number;
	talent_shot: number;
	talent_defense: number;
	learned_skills: string[];
	active_skills: string[];
	passive_skills: string[];
	modified: boolean;
}

export enum ItemTypeA {
	None = 'None',
	Weapon = 'Weapon',
	SpecialWeapon = 'SpecialWeapon',
	Armor = 'Armor',
	Accessory = 'Accessory',
	Material = 'Material',
	Consume = 'Consume',
	Ammo = 'Ammo',
	Food = 'Food',
	Essential = 'Essential',
	Glider = 'Glider',
	MonsterEquipWeapon = 'MonsterEquipWeapon',
	Blueprint = 'Blueprint'
}

export enum ItemTypeB {
	None = 'None',
	WeaponMelee = 'WeaponMelee',
	WeaponBow = 'WeaponBow',
	WeaponCrossbow = 'WeaponCrossbow',
	WeaponHandgun = 'WeaponHandgun',
	WeaponAssaultRifle = 'WeaponAssaultRifle',
	WeaponSniperRifle = 'WeaponSniperRifle',
	WeaponRocketLauncher = 'WeaponRocketLauncher',
	WeaponShotgun = 'WeaponShotgun',
	WeaponFlameThrower = 'WeaponFlameThrower',
	WeaponGatlingGun = 'WeaponGatlingGun',
	WeaponCollectionTool = 'WeaponCollectionTool',
	WeaponThrowObject = 'WeaponThrowObject',
	WeaponGrapplingGun = 'WeaponGrapplingGun',
	SPWeaponCaptureBall = 'SPWeaponCaptureBall',
	SPWeaponDamageTrap = 'SPWeaponDamageTrap',
	SPWeaponCaptureTrap = 'SPWeaponCaptureTrap',
	SPWeaponCaptureRope = 'SPWeaponCaptureRope',
	ArmorHead = 'ArmorHead',
	ArmorBody = 'ArmorBody',
	Accessory = 'Accessory',
	MaterialOre = 'MaterialOre',
	MaterialJewelry = 'MaterialJewelry',
	MaterialIngot = 'MaterialIngot',
	MaterialWood = 'MaterialWood',
	MaterialStone = 'MaterialStone',
	MaterialProccessing = 'MaterialProccessing',
	MaterialMonster = 'MaterialMonster',
	MaterialPalEgg = 'MaterialPalEgg',
	ConsumeBandage = 'ConsumeBandage',
	ConsumeSeed = 'ConsumeSeed',
	ConsumeBullet = 'ConsumeBullet',
	ConsumeWazaMachine = 'ConsumeWazaMachine',
	ConsumeTechnologyBook = 'ConsumeTechnologyBook',
	ConsumeAncientTechnologyBook = 'ConsumeAncientTechnologyBook',
	ConsumeOther = 'ConsumeOther',
	ConsumeGainStatusPoints = 'ConsumeGainStatusPoints',
	ConsumePalLevelUp = 'ConsumePalLevelUp',
	ConsumePalGainExp = 'ConsumePalGainExp',
	ConsumePalTalentUp = 'ConsumePalTalentUp',
	ConsumePalRankUp = 'ConsumePalRankUp',
	FoodMeat = 'FoodMeat',
	FoodVegetable = 'FoodVegetable',
	FoodFish = 'FoodFish',
	FoodDishMeat = 'FoodDishMeat',
	FoodDishVegetable = 'FoodDishVegetable',
	FoodDishFish = 'FoodDishFish',
	FoodProcessed = 'FoodProcessed',
	Essential = 'Essential',
	Essential_UnlockPlayerFuture = 'Essential_UnlockPlayerFuture',
	Glider = 'Glider',
	Shield = 'Shield',
	Money = 'Money',
	Medicine = 'Medicine',
	Drug = 'Drug',
	MonsterEquipWeapon = 'MonsterEquipWeapon',
	Blueprint = 'Blueprint',
	ReturnToBaseCamp = 'ReturnToBaseCamp',
	Essential_PalGear = 'Essential_PalGear'
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
	key: string;
	slot_num: number;
	state?: EntryState;
}

export type PalPresetConfig = { [K in keyof PalPreset]: boolean };

export type PalPresetPropertyNames = keyof PalPresetConfig;
export type PalPresetNameDescriptionText = { label: string; description: string };

export const palPresetNameDescriptionMap: Record<keyof PalPreset, PalPresetNameDescriptionText> = {
	is_lucky: { label: 'Lucky', description: 'Apply Lucky to preset' },
	is_boss: { label: 'Boss', description: 'Apply Boss to preset' },
	gender: { label: 'Gender', description: 'Apply Gender to preset' },
	rank_hp: { label: 'HP Souls', description: 'Apply HP Souls to preset' },
	rank_attack: { label: 'Attack Souls', description: 'Apply Attack Souls to preset' },
	rank_defense: { label: 'Defense Souls', description: 'Apply Defense Souls to preset' },
	rank_craftspeed: { label: 'Craft Speed Souls', description: 'Apply Craft Speed Souls to preset' },
	talent_hp: { label: 'HP IV', description: 'Apply HP IV to preset' },
	talent_shot: { label: 'Shot IV', description: 'Apply Shot IV to preset' },
	talent_defense: { label: 'Defense IV', description: 'Apply Defense IV to preset' },
	rank: { label: 'Rank', description: 'Apply Rank to preset' },
	level: { label: 'Level', description: 'Apply Level to preset' },
	learned_skills: { label: 'Learned Skills', description: 'Apply Learned Skills to preset' },
	active_skills: { label: 'Active Skills', description: 'Apply Active Skills to preset' },
	passive_skills: { label: 'Passive Skills', description: 'Apply Passive Skills to preset' },
	work_suitability: { label: 'Work Suitability', description: 'Apply Work Suitability to preset' },
	sanity: { label: 'Sanity', description: 'Apply Sanity to preset' },
	exp: { label: 'EXP', description: 'Apply EXP to preset' },
	lock: {
		label: 'Lock Pal',
		description: 'Lock specific Pal to preset, can only be used on this Pal'
	},
	lock_element: {
		label: 'Lock Element',
		description: `Lock the element type of the Pal in this preset. This will restrict the Pal to only being of the specified element types.`
	},
	element: {
		label: 'Element Type',
		description: `Specify the element type for the Pal in this preset. This will restrict the Pal to only being of the specified element types.`
	},
	character_id: { label: 'Pal', description: '' }
};

export const defaultPresetConfig: PalPresetConfig = {
	lock: false,
	lock_element: false,
	character_id: false,
	is_lucky: true,
	is_boss: true,
	gender: true,
	rank_hp: true,
	rank_attack: true,
	rank_defense: true,
	rank_craftspeed: true,
	talent_hp: true,
	talent_shot: true,
	talent_defense: true,
	rank: true,
	level: true,
	learned_skills: true,
	active_skills: true,
	passive_skills: true,
	work_suitability: true,
	sanity: true,
	exp: true
};

export type PalPreset = {
	lock: boolean;
	lock_element?: boolean;
	element?: ElementType;
	character_id?: string;
	is_lucky?: boolean;
	is_boss?: boolean;
	gender?: PalGender;
	rank_hp?: number;
	rank_attack?: number;
	rank_defense?: number;
	rank_craftspeed?: number;
	talent_hp?: number;
	talent_shot?: number;
	talent_defense?: number;
	rank?: number;
	level?: number;
	learned_skills?: string[];
	active_skills?: string[];
	passive_skills?: string[];
	work_suitability?: Record<WorkSuitability, number>;
	sanity?: number;
	exp?: number;
};

export interface PresetProfile {
	name: string;
	type: 'inventory' | 'active_skills' | 'passive_skills' | 'storage' | 'pal_preset';
	skills?: string[];
	common_container?: ItemContainerSlot[];
	essential_container?: ItemContainerSlot[];
	weapon_load_out_container?: ItemContainerSlot[];
	player_equipment_armor_container?: ItemContainerSlot[];
	food_equip_container?: ItemContainerSlot[];
	storage_container?: { key: string; slots: ItemContainerSlot[] };
	pal_preset?: PalPreset;
}

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
export type DynamicItemClass = 'armor' | 'weapon' | 'egg';
export type ItemGroup =
	| 'Accessory'
	| 'Body'
	| 'Common'
	| 'Food'
	| 'Glider'
	| 'Head'
	| 'Shield'
	| 'Weapon'
	| 'KeyItem'
	| 'SphereModule';

export interface DynamicItemDetails {
	durability: number;
	magazine_size?: number;
	type: DynamicItemClass;
	passive_skills?: string[];
	character_ids?: string[];
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
	disabled?: boolean;
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

export type Scaling = { hp: number; attack: number; defense: number };

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
export interface PalData {
	localized_name: string;
	description: string;
	is_pal: boolean;
	tribe: string;
	pal_deck_index: number;
	size: string;
	rarity: number;
	element_types: ElementType[];
	genus_category: string;
	organization: string;
	weapon: string;
	weapon_equip: boolean;
	scaling: Scaling;
	friendship_hp: number;
	friendship_shotattack: number;
	friendship_defense: number;
	friendship_craftspeed: number;
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
	disabled: boolean;
}

export interface SkillEffect {
	type: EffectType;
	value: number;
	target: TargetType;
}

export enum TargetType {
	ToSelf = 'ToSelf',
	ToTrainer = 'ToTrainer',
	ToSelfAndTrainer = 'ToSelfAndTrainer',
	ToBaseCampPal = 'ToBaseCampPal',
	ToBuildObject = 'ToBuildObject',
	EPalPassiveSkillEffectTargetType_MAX = 'EPalPassiveSkillEffectTargetType_MAX',
	NONE = 'None'
}

export enum EffectType {
	no = 'no',
	MaxHP = 'MaxHP',
	MeleeAttack = 'MeleeAttack',
	ShotAttack = 'ShotAttack',
	Defense = 'Defense',
	Support = 'Support',
	CraftSpeed = 'CraftSpeed',
	MoveSpeed = 'MoveSpeed',
	Homing = 'Homing',
	Explosive = 'Explosive',
	BulletSpeed = 'BulletSpeed',
	BulletAccuracy = 'BulletAccuracy',
	Recoil = 'Recoil',
	ElementFire = 'ElementFire',
	ElementWater = 'ElementWater',
	ElementLeaf = 'ElementLeaf',
	ElementElectricity = 'ElementElectricity',
	ElementIce = 'ElementIce',
	ElementEarth = 'ElementEarth',
	ElementDark = 'ElementDark',
	ElementDragon = 'ElementDragon',
	ElementResist_Normal = 'ElementResist_Normal',
	ElementResist_Fire = 'ElementResist_Fire',
	ElementResist_Water = 'ElementResist_Water',
	ElementResist_Leaf = 'ElementResist_Leaf',
	ElementResist_Electricity = 'ElementResist_Electricity',
	ElementResist_Ice = 'ElementResist_Ice',
	ElementResist_Earth = 'ElementResist_Earth',
	ElementResist_Dark = 'ElementResist_Dark',
	ElementResist_Dragon = 'ElementResist_Dragon',
	ElementBoost_Normal = 'ElementBoost_Normal',
	ElementBoost_Fire = 'ElementBoost_Fire',
	ElementBoost_Water = 'ElementBoost_Water',
	ElementBoost_Leaf = 'ElementBoost_Leaf',
	ElementBoost_Electricity = 'ElementBoost_Electricity',
	ElementBoost_Ice = 'ElementBoost_Ice',
	ElementBoost_Earth = 'ElementBoost_Earth',
	ElementBoost_Dark = 'ElementBoost_Dark',
	ElementBoost_Dragon = 'ElementBoost_Dragon',
	ElementAddItemDrop_Normal = 'ElementAddItemDrop_Normal',
	ElementAddItemDrop_Fire = 'ElementAddItemDrop_Fire',
	ElementAddItemDrop_Water = 'ElementAddItemDrop_Water',
	ElementAddItemDrop_Leaf = 'ElementAddItemDrop_Leaf',
	ElementAddItemDrop_Electricity = 'ElementAddItemDrop_Electricity',
	ElementAddItemDrop_Ice = 'ElementAddItemDrop_Ice',
	ElementAddItemDrop_Earth = 'ElementAddItemDrop_Earth',
	ElementAddItemDrop_Dark = 'ElementAddItemDrop_Dark',
	ElementAddItemDrop_Dragon = 'ElementAddItemDrop_Dragon',
	MoveSpeed_Ground = 'MoveSpeed_Ground',
	MoveSpeed_Wood = 'MoveSpeed_Wood',
	MoveSpeed_Grass = 'MoveSpeed_Grass',
	MoveSpeed_Stone = 'MoveSpeed_Stone',
	MoveSpeed_Water = 'MoveSpeed_Water',
	MoveSpeed_Snow = 'MoveSpeed_Snow',
	MoveSpeed_Lava = 'MoveSpeed_Lava',
	CollectItem = 'CollectItem',
	Mute = 'Mute',
	Logging = 'Logging',
	Mining = 'Mining',
	GainItemDrop = 'GainItemDrop',
	CollectItemDrop = 'CollectItemDrop',
	LifeSteal = 'LifeSteal',
	TemperatureResist_Heat = 'TemperatureResist_Heat',
	TemperatureResist_Cold = 'TemperatureResist_Cold',
	TemperatureInvalid_Heat = 'TemperatureInvalid_Heat',
	TemperatureInvalid_Cold = 'TemperatureInvalid_Cold',
	MaxInventoryWeight = 'MaxInventoryWeight',
	FullStomatch_Decrease = 'FullStomatch_Decrease',
	Sanity_Decrease = 'Sanity_Decrease',
	BodyPartsWeakDamage = 'BodyPartsWeakDamage',
	NonKilling = 'NonKilling',
	ItemWeightReduction = 'ItemWeightReduction',
	PalExp_Increase = 'PalExp_Increase',
	PalSP_Increase = 'PalSP_Increase',
	ShopBuyPrice_Money_Increase = 'ShopBuyPrice_Money_Increase',
	ShopSellPrice_Money_Increase = 'ShopSellPrice_Money_Increase',
	BreedSpeed = 'BreedSpeed',
	Nocturnal = 'Nocturnal',
	JumpPower_Increase = 'JumpPower_Increase',
	JumpCount_Increase = 'JumpCount_Increase',
	PalEggHatchingSpeed = 'PalEggHatchingSpeed',
	FarmCropGrowupSpeed = 'FarmCropGrowupSpeed',
	SyncroPassiveWhenCapture = 'SyncroPassiveWhenCapture',
	ActiveSkillCoolTime_Decrease = 'ActiveSkillCoolTime_Decrease',
	EPalPassiveSkillEffectType_MAX = 'EPalPassiveSkillEffectType_MAX',
	NONE = 'None'
}

export enum PalGender {
	MALE = 'Male',
	FEMALE = 'Female'
}

export type SkillType = 'Active' | 'Passive' | 'Empty';

export interface ActiveSkillDetails {
	type: string;
	element: string;
	power: number;
	cool_time: number;
	min_range: number;
	max_range: number;
}

export interface Skill {
	id: string;
	localized_name: string;
	description: string;
}

export interface ActiveSkill extends Skill {
	details: ActiveSkillDetails;
}

export const passiveSkillTier = (tier: number): string => {
	switch (tier) {
		case 3:
			return 'A';
		case 2:
			return 'B';
		case 1:
			return 'C';
		case -1:
			return 'X';
		case -2:
			return 'Y';
		default:
			return 'Z';
	}
};
export interface PassiveSkillDetails {
	rank: number;
	effects: SkillEffect[];
}

export interface PassiveSkill extends Skill {
	details: PassiveSkillDetails;
}

export type LabResearch = Technology;

export type TreeNode = {
	id: string;
	research: LabResearch;
	children: TreeNode[];
	isUnlocked: boolean;
	isCompleted: boolean;
	workAmount: number;
	totalWorkAmount: number;
};

export interface TechnologyDetails {
	category?: string;
	sub_category?: string;
	unlock_build_objects: string[];
	unlock_item_recipes: string[];
	icon_name: string;
	require_defeat_tower_boss: string;
	require_technology: string;
	require_research_id: string;
	is_boss_technology: boolean;
	level_cap: number;
	tier: number;
	cost: number;
	icon: string;
	materials?: { id: string; count: number }[];
	effect_type?: string;
	effect_value?: number;
	effect_work_suitability?: string;
	effect_item_type?: string;
	work_amount?: number;
}

export interface Technology {
	id: string;
	localized_name: string;
	description: string;
	details: TechnologyDetails;
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
	localized_name: string;
};

export enum BuildingTypeA {
	Product = 'Product',
	Pal = 'Pal',
	Storage = 'Storage',
	Food = 'Food',
	Infrastructure = 'Infrastructure',
	Light = 'Light',
	Foundation = 'Foundation',
	Defense = 'Defense',
	Other = 'Other',
	Furniture = 'Furniture',
	Dismantle = 'Dismantle',
	EPalBuildObjectTypeA_MAX = 'EPalBuildObjectTypeA_MAX'
}

export enum BuildingTypeB {
	Prod_Craft = 'Prod_Craft',
	Prod_Resource = 'Prod_Resource',
	Prod_Furnace = 'Prod_Furnace',
	Prod_Medicine = 'Prod_Medicine',
	Pal_Capture = 'Pal_Capture',
	Pal_Breed = 'Pal_Breed',
	Pal_Modify = 'Pal_Modify',
	Infra_Medical = 'Infra_Medical',
	Infra_Storage = 'Infra_Storage',
	Infra_Trade = 'Infra_Trade',
	Infra_GeneratePower = 'Infra_GeneratePower',
	Infra_Defense = 'Infra_Defense',
	Infra_Environment = 'Infra_Environment',
	Food_Basic = 'Food_Basic',
	Food_Agriculture = 'Food_Agriculture',
	Food_Cooking = 'Food_Cooking',
	Food_Livestock = 'Food_Livestock',
	Found_Basic = 'Found_Basic',
	Found_House = 'Found_House',
	Other = 'Other',
	EPalBuildObjectTypeB_MAX = 'EPalBuildObjectTypeB_MAX'
}

export type Building = {
	localized_name: string;
	type_a: BuildingTypeA;
	type_b: BuildingTypeB;
	rank: number;
	required_work_amount: number;
	required_energy_type: string;
	consume_energy_speed: number;
	materials: { id: string; count: number }[];
	material_type: string;
	material_sub_type: string;
	hp: number;
	defense: number;
	deterioration_damage: number;
	icon: string;
};

export enum EntryState {
	NONE = 'None',
	MODIFIED = 'Modified',
	NEW = 'New',
	DELETED = 'Deleted'
}

export interface CloneToUpsModalProps {
	collectionId?: number;
	tags: string[];
	notes: string;
}

export interface ImportToUpsModalResults {
	sourceType: 'pal_box' | 'gps' | 'dps';
	sourceSlot?: number;
	collectionId?: number;
	tags: string[];
	notes: string;
	palId: string;
	playerId?: string;
}
