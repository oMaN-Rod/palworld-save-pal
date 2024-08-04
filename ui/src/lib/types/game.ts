export type MovesetKey = `EPalWazaID::${string}`;

export type Moveset = {
	[K in MovesetKey]?: number;
};

export type Scaling = {
	HP: number;
	Attack: number;
	Defense: number;
};

export type Suitabilities = {
	[key: string]: number;
};

export interface PalData {
	CodeName: string;
	Type: ElementType[];
	Moveset: Moveset;
	RaidMoveset?: Moveset;
	Scaling: Scaling;
	Suitabilities: Suitabilities;
	Tower?: boolean;
	Human?: boolean;
	Bonuses?: Bonuses;
}

export interface Bonuses {
	Attack: number;
	Defense: number;
	WorkSpeed: number;
}

enum PalGender {
	UNKNOWN = 'Unknown',
	MALE = 'male',
	FEMALE = 'female'
}

type SkillType = 'Active' | 'Passive' | 'Empty';

interface ActiveSkillDetails {
	Type: string;
	Power: number;
	CT: number;
	Name: string;
	Description: string;
	Exclusive?: string[];
}

interface Skill {
	id: string;
	name: string;
}

interface ActiveSkill extends Skill {
	details: ActiveSkillDetails;
}

interface PassiveSkillDetails {
	Description: string;
	Effect: string;
	Rating: string;
	Tier: string;
	Bonuses: Bonuses;
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

type PalSummary = {
	instance_id: string;
	character_id: string;
	owner_uid: string;
	name: string;
	nickname: string;
	level: number;
	elements: ElementType[];
};

type Element = {
	Name: string;
	Color: string;
	Icon: string;
	IconBadge: string;
	IconEgg: string;
	IconFruit: string;
	IconWhite: string;
};

type Player = {
	uid: string;
	nickname: string;
	level: number;
	pals: Record<string, PalSummary>;
};

type SaveFile = {
	name: string;
	size: number;
};

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
	storage_id?: string;
	storage_slot: number;
	learned_skills: string[];
	active_skills: string[];
	passive_skills: string[];
	work_suitabilities: Record<WorkSuitability, number>;
	hp: number;
	max_hp: number;
	elements: ElementType[];
};

// const defaultPal: Pal = {
// 	name: '',
// 	character_id: '',
// 	instance_id: '',
// 	owner_uid: '',
// 	is_lucky: false,
// 	is_boss: false,
// 	gender: PalGender.UNKNOWN,
// 	work_speed: 0.0,
// 	talent_hp: 0,
// 	talent_melee: 0,
// 	talent_shot: 0,
// 	talent_defense: 0,
// 	rank: 1,
// 	level: 1,
// 	nickname: '',
// 	is_tower: false,
// 	storage_slot: 0,
// 	learned_skills: [],
// 	active_skills: [],
// 	passive_skills: [],
// 	work_suitabilities: {} as Record<WorkSuitability, number>,
// 	hp: 0,
// 	max_hp: 0,
// 	elements: []
// };

export { PalGender };
export type {
	ActiveSkill,
	ActiveSkillDetails,
	Element,
	ElementType,
	Pal,
	PalSummary,
	PassiveSkill,
	PassiveSkillDetails,
	Player,
	SaveFile,
	WorkSuitability,
	SkillType,
	Skill
};

