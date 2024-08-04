import { PalGender, type Pal, type Player, type WorkSuitability } from '$types';

const fullSuitabilitySet: Record<WorkSuitability, number> = {
	EmitFlame: 0,
	Watering: 0,
	Seeding: 0,
	GenerateElectricity: 0,
	Handcraft: 0,
	Collection: 0,
	Deforest: 0,
	Mining: 0,
	OilExtraction: 0,
	ProductMedicine: 0,
	Cool: 0,
	Transport: 0,
	MonsterFarm: 0
};

const players: Record<string, Player> = {
	'123e4567-e89b-12d3-a456-426614174000': {
		uid: '123e4567-e89b-12d3-a456-426614174000',
		nickname: 'Alice',
		pal_summary: {
			'11111111-1111-1111-1111-111111111111': {
				instance_id: '11111111-1111-1111-1111-111111111111',
				character_id: 'Foxparks',
				owner_uid: '123e4567-e89b-12d3-a456-426614174000',
				name: 'Foxparks',
				nickname: 'Sparky',
				level: 15,
				elements: ['Fire']
			},
			'22222222-2222-2222-2222-222222222222': {
				instance_id: '22222222-2222-2222-2222-222222222222',
				character_id: 'Fuack',
				owner_uid: '123e4567-e89b-12d3-a456-426614174000',
				name: 'Fuack',
				nickname: 'Splash',
				level: 12,
				elements: ['Water']
			}
		}
	},
	'987fcdeb-54ba-4321-8f01-554288721333': {
		uid: '987fcdeb-54ba-4321-8f01-554288721333',
		nickname: 'Bob',
		pal_summary: {
			'33333333-3333-3333-3333-333333333333': {
				instance_id: '33333333-3333-3333-3333-333333333333',
				character_id: 'Lifmunk',
				owner_uid: '987fcdeb-54ba-4321-8f01-554288721333',
				name: 'Lifmunk',
				nickname: 'Whispy',
				level: 18,
				elements: ['Grass']
			},
			'44444444-4444-4444-4444-444444444444': {
				instance_id: '44444444-4444-4444-4444-444444444444',
				character_id: 'Grizzbolt',
				owner_uid: '987fcdeb-54ba-4321-8f01-554288721333',
				name: 'Grizzbolt',
				nickname: 'Zappy',
				level: 20,
				elements: ['Electric']
			}
		}
	},
	'456abcde-f123-9876-f431-987654321000': {
		uid: '456abcde-f123-9876-f431-987654321000',
		nickname: 'Charlie',
		pal_summary: {
			'55555555-5555-5555-5555-555555555555': {
				instance_id: '55555555-5555-5555-5555-555555555555',
				character_id: 'Digtoise',
				owner_uid: '456abcde-f123-9876-f431-987654321000',
				name: 'Digtoise',
				nickname: 'Pebbles',
				level: 14,
				elements: ['Ground']
			},
			'66666666-6666-6666-6666-666666666666': {
				instance_id: '66666666-6666-6666-6666-666666666666',
				character_id: 'Vanwyrm',
				owner_uid: '456abcde-f123-9876-f431-987654321000',
				name: 'Vanwyrm',
				nickname: 'Drakey',
				level: 22,
				elements: ['Fire', 'Dark']
			},
			'77777777-7777-7777-7777-777777777777': {
				instance_id: '77777777-7777-7777-7777-777777777777',
				character_id: 'Frostallion',
				owner_uid: '456abcde-f123-9876-f431-987654321000',
				name: 'Frostallion',
				nickname: 'Blizzard',
				level: 30,
				elements: ['Ice']
			}
		}
	}
};

const pals: Record<string, Pal> = {
	'11111111-1111-1111-1111-111111111111': {
		name: 'Foxparks',
		instance_id: '11111111-1111-1111-1111-111111111111',
		owner_uid: '123e4567-e89b-12d3-a456-426614174000',
		is_lucky: false,
		is_boss: false,
		character_id: 'Kitsunebi',
		gender: PalGender.FEMALE,
		work_speed: 100,
		talent_hp: 50,
		talent_melee: 60,
		talent_shot: 55,
		talent_defense: 45,
		rank: 1,
		level: 15,
		nickname: 'Sparky',
		is_tower: false,
		storage_slot: 1,
		learned_skills: ['FireBlast', 'FireSeed', 'FlareArrow'],
		active_skills: ['FireBlast', 'FireSeed'],
		passive_skills: ['ElementBoost_Fire_1_PAL'],
		work_suitabilities: {
			...fullSuitabilitySet,
			EmitFlame: 1
		},
		hp: 100,
		max_hp: 100,
		elements: ['Fire']
	},
	'22222222-2222-2222-2222-222222222222': {
		name: 'Fuack',
		instance_id: '22222222-2222-2222-2222-222222222222',
		owner_uid: '123e4567-e89b-12d3-a456-426614174000',
		is_lucky: false,
		is_boss: false,
		character_id: 'BluePlatypus',
		gender: PalGender.MALE,
		work_speed: 90,
		talent_hp: 55,
		talent_melee: 50,
		talent_shot: 65,
		talent_defense: 50,
		rank: 1,
		level: 12,
		nickname: 'Splash',
		is_tower: false,
		storage_slot: 2,
		learned_skills: ['WaterGun', 'AquaJet', 'BubbleShot'],
		active_skills: ['WaterGun', 'AquaJet'],
		passive_skills: ['ElementBoost_Aqua_1_PAL'],
		work_suitabilities: {
			...fullSuitabilitySet,
			Watering: 1,
			Handcraft: 1
		},
		hp: 90,
		max_hp: 90,
		elements: ['Water']
	},
	'33333333-3333-3333-3333-333333333333': {
		name: 'Lifmunk',
		instance_id: '33333333-3333-3333-3333-333333333333',
		owner_uid: '987fcdeb-54ba-4321-8f01-554288721333',
		is_lucky: true,
		is_boss: false,
		character_id: 'Carbunclo',
		gender: PalGender.FEMALE,
		work_speed: 110,
		talent_hp: 45,
		talent_melee: 55,
		talent_shot: 70,
		talent_defense: 50,
		rank: 2,
		level: 18,
		nickname: 'Whispy',
		is_tower: false,
		storage_slot: 1,
		learned_skills: ['WindCutter', 'SeedMachinegun', 'GrassTornado'],
		active_skills: ['WindCutter', 'SeedMachinegun'],
		passive_skills: ['ElementBoost_Leaf_1_PAL', 'CraftSpeed_up1'],
		work_suitabilities: {
			...fullSuitabilitySet,
			Seeding: 1,
			Handcraft: 1,
			Collection: 1
		},
		hp: 110,
		max_hp: 110,
		elements: ['Grass']
	},
	'44444444-4444-4444-4444-444444444444': {
		name: 'Grizzbolt',
		instance_id: '44444444-4444-4444-4444-444444444444',
		owner_uid: '987fcdeb-54ba-4321-8f01-554288721333',
		is_lucky: false,
		is_boss: false,
		character_id: 'ElecPanda',
		gender: PalGender.MALE,
		work_speed: 95,
		talent_hp: 70,
		talent_melee: 75,
		talent_shot: 65,
		talent_defense: 60,
		rank: 3,
		level: 20,
		nickname: 'Zappy',
		is_tower: false,
		storage_slot: 2,
		learned_skills: ['ThunderBall', 'ElecWave', 'ThreeThunder'],
		active_skills: ['ThunderBall', 'ElecWave', 'ThreeThunder'],
		passive_skills: ['ElementBoost_Thunder_2_PAL', 'PAL_ALLAttack_up1'],
		work_suitabilities: {
			...fullSuitabilitySet,
			GenerateElectricity: 3,
			Handcraft: 2,
			Deforest: 2,
			Transport: 3
		},
		hp: 150,
		max_hp: 150,
		elements: ['Electric']
	},
	'55555555-5555-5555-5555-555555555555': {
		name: 'Digtoise',
		instance_id: '55555555-5555-5555-5555-555555555555',
		owner_uid: '456abcde-f123-9876-f431-987654321000',
		is_lucky: false,
		is_boss: false,
		character_id: 'DrillGame',
		gender: PalGender.FEMALE,
		work_speed: 85,
		talent_hp: 65,
		talent_melee: 50,
		talent_shot: 60,
		talent_defense: 80,
		rank: 2,
		level: 14,
		nickname: 'Pebbles',
		is_tower: false,
		storage_slot: 1,
		learned_skills: ['MudShot', 'StoneShotgun', 'SandTornado'],
		active_skills: ['MudShot', 'StoneShotgun'],
		passive_skills: ['ElementResist_Earth_1_PAL', 'Deffence_up1'],
		work_suitabilities: {
			...fullSuitabilitySet,
			Mining: 3
		},
		hp: 120,
		max_hp: 120,
		elements: ['Ground']
	},
	'66666666-6666-6666-6666-666666666666': {
		name: 'Vanwyrm',
		instance_id: '66666666-6666-6666-6666-666666666666',
		owner_uid: '456abcde-f123-9876-f431-987654321000',
		is_lucky: false,
		is_boss: true,
		character_id: 'BirdDragon',
		gender: PalGender.MALE,
		work_speed: 100,
		talent_hp: 75,
		talent_melee: 80,
		talent_shot: 85,
		talent_defense: 70,
		rank: 4,
		level: 22,
		nickname: 'Drakey',
		is_tower: false,
		storage_slot: 2,
		learned_skills: ['FireBlast', 'DarkBall', 'Flamethrower', 'ShadowBall'],
		active_skills: ['FireBlast', 'DarkBall', 'Flamethrower', 'ShadowBall'],
		passive_skills: ['ElementBoost_Fire_2_PAL', 'ElementBoost_Dark_1_PAL', 'MoveSpeed_up_2'],
		work_suitabilities: {
			...fullSuitabilitySet,
			EmitFlame: 1,
			Transport: 3
		},
		hp: 200,
		max_hp: 200,
		elements: ['Fire', 'Dark']
	},
	'77777777-7777-7777-7777-777777777777': {
		name: 'Frostallion',
		instance_id: '77777777-7777-7777-7777-777777777777',
		owner_uid: '456abcde-f123-9876-f431-987654321000',
		is_lucky: false,
		is_boss: true,
		character_id: 'IceHorse',
		gender: PalGender.MALE,
		work_speed: 100,
		talent_hp: 75,
		talent_melee: 80,
		talent_shot: 85,
		talent_defense: 70,
		rank: 4,
		level: 30,
		nickname: 'Blizzard',
		is_tower: false,
		storage_slot: 2,
		learned_skills: [
			'EPalWazaID::IceMissile',
			'EPalWazaID::Unique_IceHorse_IceBladeAttack',
			'EPalWazaID::FrostBreath'
		],
		active_skills: [
			'EPalWazaID::IceMissile',
			'EPalWazaID::Unique_IceHorse_IceBladeAttack',
			'EPalWazaID::FrostBreath'
		],
		passive_skills: ['ElementBoost_Fire_2_PAL', 'ElementBoost_Dark_1_PAL', 'MoveSpeed_up_2'],
		work_suitabilities: {
			...fullSuitabilitySet,
			Cool: 4
		},
		hp: 200,
		max_hp: 200,
		elements: ['Ice']
	}
};

export { pals, players };
