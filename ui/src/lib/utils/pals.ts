import { expData, palsData } from '$lib/data';
import { getStats } from '$lib/utils';
import { getAppState } from '$states';
import {
	EntryState,
	type Pal,
	type Player,
	type PresetProfile,
	type WorkSuitability
} from '$types';

const prefixTypeMap = [
	['predator_', 'Predator'],
	['summon_', 'Summon'],
	['raid_', 'Raid'],
	['gym_', 'Tower Boss']
] as const;

export function canBeLucky(character_id: string): [string, boolean] {
	const lowerCaseId = character_id.toLowerCase();
	for (const [prefix, type] of prefixTypeMap) {
		if (lowerCaseId.includes(prefix)) {
			return [type, false];
		}
	}

	return ['', true];
}

export function canBeAlpha(character_id: string): [string, boolean] {
	const lowerCaseId = character_id.toLowerCase();
	const excludedPrefixes = [
		...prefixTypeMap,
		['yakushimamonster', 'This'],
		['yakushimaboss001_small', 'This'],
		['quest_farmer03_', 'This']
	];

	for (const [prefix, type] of excludedPrefixes) {
		if (lowerCaseId.startsWith(prefix)) {
			return [type, false];
		}
	}

	return ['', true];
}

export function formatNickname(nickname: string, prefix: string | undefined) {
	if (prefix && !nickname.startsWith(prefix)) {
		return `${prefix} ${nickname}`;
	}
	return nickname;
}

export async function handleMaxOutPal(pal: Pal, player: Player): Promise<void> {
	if (!pal) return;
	const appState = getAppState();
	pal.level = appState.settings.cheat_mode ? 255 : 80;
	const maxLevelData = expData.expData[appState.settings.cheat_mode ? '100' : '81'];
	pal.exp = maxLevelData.PalTotalEXP - maxLevelData.PalNextEXP;
	const [_, valid] = canBeLucky(pal.character_id);
	pal.is_boss = valid;
	pal.is_lucky = false;
	pal.talent_hp = appState.settings.cheat_mode ? 255 : 100;
	pal.talent_shot = appState.settings.cheat_mode ? 255 : 100;
	pal.talent_defense = appState.settings.cheat_mode ? 255 : 100;
	pal.rank = appState.settings.cheat_mode ? 255 : 5;
	pal.rank_hp = appState.settings.cheat_mode ? 255 : 20;
	pal.rank_defense = appState.settings.cheat_mode ? 255 : 20;
	pal.rank_attack = appState.settings.cheat_mode ? 255 : 20;
	pal.rank_craftspeed = appState.settings.cheat_mode ? 255 : 20;
	getStats(pal, player);
	pal.hp = pal.max_hp;
	pal.state = EntryState.MODIFIED;
	const palData = palsData.getByKey(pal.character_key);
	if (palData) {
		pal.stomach = palData.max_full_stomach;
		for (const [key, value] of Object.entries(palData.work_suitability)) {
			if (value === 0) continue;
			pal.work_suitability[key as WorkSuitability] = Math.min(5 - value, 4);
		}
	} else {
		pal.stomach = 150;
	}
	pal.friendship_point = 200000;
}

export const applyPalPreset = (pal: Pal, presetProfile: PresetProfile, player?: Player): void => {
	if (!presetProfile.pal_preset) return;

	const palData = palsData.getByKey(pal.character_key);
	if (!palData) return;

	const skipKeys = new Set(['character_id', 'character_key', 'lock', 'lock_element', 'element']);
	const [, canBeBossValue] = canBeLucky(pal.character_id);

	for (const [key, value] of Object.entries(presetProfile.pal_preset)) {
		if (skipKeys.has(key) || value == null) continue;

		if (key === 'is_boss' || key === 'is_lucky') {
			handleBossLuckyFlags(pal, palData, key, value as boolean, canBeBossValue);
		} else {
			(pal as Record<string, any>)[key] = value;
		}
	}

	getStats(pal, player);
	pal.hp = pal.max_hp;
	pal.stomach = palData.max_full_stomach;
	pal.state = EntryState.MODIFIED;
};

function handleBossLuckyFlags(
	pal: Pal,
	palData: any,
	key: 'is_boss' | 'is_lucky',
	value: boolean,
	canBeBossValue: boolean
): void {
	if (!canBeBossValue || !palData.is_pal) {
		pal.is_boss = false;
		pal.is_lucky = false;
		return;
	}

	if (key === 'is_boss') {
		pal.is_boss = value;
		if (value) pal.is_lucky = false;
	} else {
		pal.is_lucky = value;
		if (value) pal.is_boss = false;
	}
}

export enum WazaID {
	NONE = 0,
	Human_Punch = 1,
	WorkAttack = 2,
	Throw = 3,
	Scratch = 4,
	EnergyShot = 5,
	Unique_Anubis_LowRoundKick = 6,
	Unique_Anubis_GroundPunch = 7,
	Unique_Anubis_Tackle = 8,
	Unique_Deer_PushupHorn = 9,
	HyperBeam = 10,
	PowerShot = 11,
	PowerBall = 12,
	Unique_Garm_Bite = 13,
	Intimidate = 14,
	Unique_Boar_Tackle = 15,
	Unique_PinkCat_CatPunch = 16,
	Unique_FlowerDinosaur_Whip = 17,
	Unique_SheepBall_Roll = 18,
	Unique_ChickenPal_ChickenPeck = 19,
	Unique_Gorilla_GroundPunch = 20,
	Unique_Grassmammoth_Earthquake = 21,
	AirCanon = 22,
	Unique_GrassPanda_MusclePunch = 23,
	Unique_RobinHood_BowSnipe = 24,
	Unique_Alpaca_Tackle = 25,
	Unique_KingAlpaca_BodyPress = 26,
	Unique_Werewolf_Scratch = 27,
	Unique_FengyunDeeper_CloudTempest = 28,
	Unique_Baphomet_SwallowKite = 29,
	Unique_HerculesBeetle_BeetleTackle = 30,
	Unique_HawkBird_Storm = 31,
	Unique_Eagle_GlidingNail = 32,
	SelfDestruct = 33,
	SelfDestruct_Bee = 34,
	SelfExplosion = 35,
	Unique_Garm_BiteV2 = 36,
	Unique_GuardianDog_Bite = 37,
	Unique_GuardianDog_BiteV2 = 38,
	RadiantBarrage = 39,
	FireBlast = 40,
	Flamethrower = 41,
	FireBall = 42,
	FlareArrow = 43,
	FireSeed = 44,
	Unique_Horus_FlareBird = 45,
	FlareTornado = 46,
	Inferno = 47,
	Unique_FireKirin_Tackle = 48,
	Unique_AmaterasuWolf_FireCharge = 49,
	Unique_VolcanicMonster_MagmaAttack = 50,
	Unique_FlameBuffalo_FlameHorn = 51,
	Eruption = 52,
	FlameWall = 53,
	FlameFunnel = 54,
	Unique_AmaterasuWolf_Bite = 55,
	Unique_AmaterasuWolf_BiteV2 = 56,
	WaterGun = 57,
	WaterWave = 58,
	HydroPump = 59,
	WaterBall = 60,
	TidalWave = 61,
	AquaJet = 62,
	BubbleShot = 63,
	AcidRain = 64,
	SeaGush = 65,
	RipTide = 66,
	DiversionLaser = 67,
	HydroSlicer = 68,
	Unique_KingWhale_HomingBubble = 69,
	Unique_KingWhale_AquaBlade = 70,
	CreepingBubble = 71,
	Unique_KingWhale_Ripple = 72,
	Unique_KingWhale_Maelstrom = 73,
	Unique_KingWhale_TidalWave = 74,
	Unique_KingWhale_AquaTornado = 75,
	Unique_KingWhale_TidalBore = 76,
	Unique_KingWhale_SuperTidalBore = 77,
	Unique_KingWhale_WaveTackle = 78,
	Unique_KingWhale_Breaching = 79,
	Unique_KingWhale_Breaching_P3 = 80,
	Unique_KingWhale_BaseCampAttack = 81,
	Unique_BluePlatypus_Toboggan = 82,
	Unique_TentacleTurtle_HydroSpin = 83,
	Unique_SakuraSaurus_Water_SplashTackle = 84,
	WindCutter = 85,
	GrassTornado = 86,
	SolarBeam = 87,
	SeedMachinegun = 88,
	SeedMine = 89,
	RootAttack = 90,
	SpecialCutter = 91,
	CrossWind = 92,
	ReflectiveShuriken = 93,
	HealingTree = 94,
	Unique_QueenBee_SpinLance = 95,
	ThunderRain = 96,
	ThunderBall = 97,
	LineThunder = 98,
	CrossThunder = 99,
	ThreeThunder = 100,
	ElecWave = 101,
	Thunderbolt = 102,
	ThunderFunnel = 103,
	SpreadPulse = 104,
	LockonLaser = 105,
	LightningStrike = 106,
	ThunderSpear = 107,
	Unique_ElecPanda_ElecScratch = 108,
	Unique_Kirin_LightningTackle = 109,
	Unique_FlowerDinosaur_Electric_ThunderWhip = 110,
	Unique_ThunderDog_Bite = 111,
	Unique_ThunderDog_BiteV2 = 112,
	IceMissile = 113,
	BlizzardLance = 114,
	SnowStorm = 115,
	IcicleThrow = 116,
	IceBlade = 117,
	Unique_IceHorse_IceBladeAttack = 118,
	Unique_IceNarwhal_JumpingHorn = 119,
	Unique_KingAlpaca_Ice_IcePress = 120,
	SandTornado = 121,
	ThrowRock = 122,
	RockLance = 123,
	MudShot = 124,
	StoneShotgun = 125,
	Unique_DrillGame_ShellAttack = 126,
	Unique_Deer_Ground_DirtyHorn = 127,
	Unique_Gorilla_Ground_EarthPunch = 128,
	Unique_GoldenHorse_Bite = 129,
	Unique_GoldenHorse_BiteV2 = 130,
	DarkLaser = 131,
	DarkWave = 132,
	ShadowBall = 133,
	Psychokinesis = 134,
	PoisonShot = 135,
	GhostFlame = 136,
	GravityShot = 137,
	Unique_DarkCrow_TelePoke = 138,
	Unique_Baphomet_Dark_DarkKite = 139,
	Unique_IceHorse_Dark_DarkBladeAttack = 140,
	Unique_AmaterasuWolf_Dark_Bite = 141,
	Unique_AmaterasuWolf_Dark_BiteV2 = 142,
	Unique_BlackPuppy_Bite = 143,
	Unique_BlackPuppy_BiteV2 = 144,
	DragonMeteor = 145,
	DragonBreath = 146,
	DragonWave = 147,
	DragonCanon = 148,
	Unique_FairyDragon_FairyTornado = 149,
	Funnel_DreamDemon = 150,
	Funnel_RaijinDaughter = 151,
	Funnel_RaijinDaughter_Water = 152,
	StardustArrow = 153,
	Tremor = 154,
	FrostBreath = 155,
	DiamondFall = 156,
	BeamSlicer = 157,
	Commet = 158,
	DarkBall = 159,
	PoisonFog = 160,
	DarkLegion = 161,
	DarkCanon = 162,
	DarkArrow = 163,
	DarkPulse = 164,
	Apocalypse = 165,
	StarMine = 166,
	AirBlade = 167,
	HolyBlast = 168,
	RootLance = 169,
	LineGeyser = 170,
	WallSplash = 171,
	TriSpark = 172,
	ThunderStorm = 173,
	SandTwister = 174,
	IcicleLine = 175,
	ThreeCommet = 176,
	CommetRain = 177,
	BlastCanon = 178,
	ChargeCanon = 179,
	RangeThunder = 180,
	Railbolt = 181,
	ShokeiLaser = 182,
	BubbleShower = 183,
	WaterBalloon = 184,
	IciclePierce = 185,
	DoubleIcicleThrow = 186,
	IceAge = 187,
	RaidCutter = 188,
	WindEdge = 189,
	FlareTwister = 190,
	TrisRing = 191,
	Unique_BirdDragon_FireBreath = 192,
	Unique_BlackMetalDragon_FirePunch = 193,
	Unique_DarkScorpion_Pierce = 194,
	Unique_GhostBeast_Tossin = 195,
	Unique_JetDragon_JumpBeam = 196,
	Unique_ThunderBird_ThunderStorm = 197,
	Unique_Yeti_SnowBall = 198,
	Unique_NaughtyCat_CatPress = 199,
	Unique_IceDeer_IceHorn = 200,
	Unique_KingBahamut_AirCrash = 201,
	Unique_Manticore_InfernoStrike = 202,
	Unique_SoldierBee_NeedleLance = 203,
	Unique_ThunderDog_InazumaShorai = 204,
	Unique_BlackCentaur_TwoSpearRushes = 205,
	Unique_BlackGriffon_TackleLaser = 206,
	Unique_SakuraSaurus_SideTackle = 207,
	Unique_ThunderDragonMan_ThunderSwordAttack = 208,
	Unique_RedArmorBird_TriplePeck = 209,
	Unique_CaptainPenguin_BodySlide = 210,
	Unique_CaptainPenguin_Black_BodySlide_Electric = 211,
	Unique_Ronin_Iai = 212,
	Unique_GrassRabbitMan_GrassRoundKick = 213,
	Unique_SaintCentaur_OneSpearRushes = 214,
	Unique_Umihebi_WindingTackle = 215,
	Unique_WeaselDragon_FlyingTackle = 216,
	Unique_WhiteTiger_IceScratch = 217,
	Unique_IceCrocodile_SpitAttack = 218,
	Unique_BirdDragon_Ice_IceBreath = 219,
	Unique_FireKirin_Dark_DarkTossin = 220,
	Unique_VolcanicMonster_Ice_IceAttack = 221,
	Unique_LeafMomonga_SomerSault = 222,
	Unique_Yeti_Grass_GrassBall = 223,
	Unique_GrassPanda_Electric_ElectricPunch = 224,
	Unique_NightLady_WarpBeam = 225,
	Unique_NightLady_WarpBeam_Straight = 226,
	Unique_NightLady_FlameNightmare = 227,
	Unique_MoonQueen_MoonBeam = 228,
	Unique_MoonQueen_MoonBlade = 229,
	Unique_KingBahamut_ArmSmash = 230,
	Unique_WingGolem_RoundCutter = 231,
	Unique_ScorpionMan_Uppercut = 232,
	Unique_FeatherOstrich_Tossin = 233,
	Unique_DarkAlien_JumpScractch = 234,
	Unique_SifuDog_Counter = 235,
	Unique_ThunderDragonMan_NumerousSwordAttack = 236,
	Unique_ElecPanda_GatlingAttack = 237,
	Unique_LilyQueen_LilyHealing = 238,
	Unique_LilyQueen_LilyHealing_Boss = 239,
	Unique_LilyQueen_WindBarrier = 240,
	Unique_Horus_PerfectStorm = 241,
	Unique_BlackGriffon_TackleLaser2 = 242,
	Unique_MoonQueen_IceMoonBlade = 243,
	Unique_DarkMechaDragon_SetFunnel = 244,
	Unique_DarkMechaDragon_ConvergentBeam = 245,
	Unique_DarkMechaDragon_FunnelLaser = 246,
	Unique_DarkMechaDragon_BeamSlash = 247,
	Unique_DarkMechaDragon_WarpComet = 248,
	Unique_Umihebi_Fire_FireWindingTackle = 249,
	Unique_PurpleSpider_SpiderRaid = 250,
	Unique_MysteryMask_LifeSteal = 251,
	Unique_GrimGirl_BrutalMachete = 252,
	Unique_SnowTigerBeastman_TrampleSlash = 253,
	Unique_SnowTigerBeastman_SnowImpact = 254,
	Unique_WhiteShieldDragon_ShieldTackle = 255,
	Unique_NightBlueHorse_DeathStep = 256,
	Unique_BlueThunderHorse_FlashDash = 257,
	Unique_WhiteDeer_HolyPillar = 258,
	Unique_GoldenHorse_StoneDash = 259,
	Unique_WhiteTiger_Ground_IronScratch = 260,
	Unique_FengyunDeeper_Electric_ThunderTempest = 261,
	Unique_Werewolf_Ice_SnowScratch = 262,
	Unique_Horus_Water_AquaStorm = 263,
	Unique_AmaterasuWolf_Dark_DarkCharge = 264,
	Unique_OctopursGirl_InkJet = 265,
	Unique_StuffedShark_HiddenWeapon = 266,
	Unique_Plesiosaur_LongBreath = 267,
	Unique_TropicalOstrich_DashKick = 268,
	Unique_GhostAnglerfish_SweepBait = 269,
	Unique_GhostAnglerfish_Fire_SweepBait_Fire = 270,
	Unique_PoseidonOrca_TorrentLaser = 271,
	Unique_VolcanoDragon_VolcanicLaser = 272,
	Unique_VolcanoDragon_MagmaSpit = 273,
	Unique_Sekhmet_RollingScratch = 274,
	Unique_Sekhmet_SomersaultScratch = 275,
	Unique_LegendDeer_WarpPillarBurst = 276,
	Unique_LegendDeer_BarrierRelease_Normal = 277,
	Unique_LegendDeer_BarrierRelease_Grass = 278,
	Unique_LegendDeer_BarrierRelease_Water = 279,
	Unique_LegendDeer_RadiantPurge = 280,
	Unique_LegendDeer_RadiantWingRush = 281,
	Unique_LegendDeer_RadiantPurge_Otomo = 282,
	PredatorBeam = 283,
	PredatorWave = 284,
	PredatorLockon = 285,
	RockBeat = 286,
	IceWall = 287,
	WindBurst = 288,
	Unique_SamuraiDog_Bite = 289,
	Unique_SamuraiDog_BiteV2 = 290,
	Unique_NightBlueHorse_Neutral_Tossin = 291,
	Unique_NightBlueHorse_Neutral_AirStep = 292,
	Unique_Kirin_Ice_IceTackle = 293,
	Unique_ThunderDog_Ice_KoriShorai = 294,
	Unique_ScorpionMan_Erectric_UpperThunder = 295,
	Unique_ThunderDog_Ice_Bite = 296,
	Unique_ThunderDog_Ice_BiteV2 = 297,
	Unique_BluePlatypus_Toboggan_Fire = 298,
	Unique_NightBlueHorse_Tossin = 299,
	Unique_BlueThunderHorse_Tossin = 300,
	Unique_MonochromeQueen_BalletJump = 301,
	Unique_CuteMole_DiggingAttack = 302,
	Unique_SamuraiDog_DashSlash = 303,
	Unique_GrassGolem_ArmCannon = 304,
	Unique_GrassGolem_RocketPunch = 305,
	Unique_SnakeGirl_SnakeShot = 306,
	Unique_MummyPal_MummyAttack = 307,
	Unique_ClownRabbit_TrickShow = 308,
	Unique_CubeTurtle_CubePress = 309,
	Unique_SumoDog_SumoStomp = 310,
	Unique_ElecSnail_ShellCharge = 311,
	Unique_LotusDragon_LotusBloom = 312,
	Unique_DomeArmorDragon_ExplosiveMissile = 313,
	Unique_GhostDragon_TailSlash = 314,
	Unique_GhostDragon_PhosphorousBeam = 315,
	Unique_GrassMinotaur_BullRush = 316,
	Unique_GrassMinotaur_Ice_BullRush = 317,
	Unique_PandaGirl_RapidKick = 318,
	Unique_LanternButler_LanternFlame = 319,
	Unique_RockBeast_RockHorn = 320,
	Unique_RockBeast_Ice_IceHorn = 321,
	Unique_ElecPomeranian_Bite = 322,
	Unique_ElecPomeranian_BiteV2 = 323,
	Unique_BlueSkyDragon_Tossin = 324,
	Unique_BlueSkyDragon_SweepBreath = 325,
	Unique_BlueSkyDragon_DrainStorm = 326,
	Unique_RedFlowerBird_JumpKick = 327,
	Unique_WhiteDeer_Dark_DarkPillar = 328,
	Unique_GrassGolem_Dark_DarkArmCannon = 329,
	Unique_WingGolem_Fire_FlameCutter = 330,
	Unique_ThunderBird_Ice_SnowStrom = 331,
	Unique_CubeTurtle_Neutral_HolyPress = 332,
	Unique_VolcanoDragon_Ice_IcicleSpit = 333,
	Unique_VolcanoDragon_Ice_IceLaser = 334,
	Unique_GrassMinotaur_BullRush_Lower = 335,
	Unique_GrassMinotaur_Ice_BullRush_Lower = 336,
	Unique_Mothman_GiantSpore = 337,
	Unique_Mothman_SporeScatter = 338,
	Unique_FlowerPrince_PoisonGasDance = 339,
	Unique_FlowerPrince_PoisonGasTackle = 340,
	Unique_WorldTreeDragon_PaldiumShot = 341,
	Unique_WorldTreeDragon_PaldiumCannon = 342,
	Unique_WorldTreeDragon_PaldiumExplosion = 343,
	Unique_WorldTreeDragon_HaloBeam = 344,
	Unique_WorldTreeDragon_BigBang = 345,
	Unique_WorldTreeDragon_Supernova = 346,
	Unique_WorldTreeDragon_PaldiumRain = 347,
	Unique_WorldTreeDragon_HaloCutter = 348,
	Unique_WorldTreeDragon_LaserGliding = 349,
	Unique_LilyQueen_GYM_Act = 350,
	Unique_ThunderDragonMan_GYM_Act = 351,
	Unique_MoonQueen_GYM_Act = 352,
	Unique_MoonQueen_GYM_Hard_Act = 353,
	Unique_BlueSkyDragon_GYM_Act = 354,
	BlueThunderHorse_PartnerSkill = 355,
	Unique_Ronin_Iai_PartnerSkill = 356,
	PoseidonOrca_PartnerSkill_SpearBullet = 357,
	PoseidonOrca_PartnerSkill = 358,
	GrassGolem_PartnerSkill = 359,
	GrassGolem_Dark_PartnerSkill = 360,
	Human_Rolling = 361,
	Weapon_Use = 362,
	Unique_Yakushima_SummonServant = 363,
	Unique_Yakushima_EyeTossin = 364,
	Unique_Yakushima_MouthTossin = 365,
	Unique_YakushimaMonster001_SlimePress_Normal = 366,
	Unique_YakushimaMonster001_SlimePress_Leaf = 367,
	Unique_YakushimaMonster001_SlimePress_Water = 368,
	Unique_YakushimaMonster001_SlimePress_Fire = 369,
	Unique_YakushimaMonster001_SlimePress_Dark = 370,
	Unique_YakushimaMonster001_SlimePress_Rainbow = 371,
	Unique_YakushimaBoss001_Small_DemonEyeCharge = 372,
	Unique_YakushimaMonster002_SwordCharge = 373,
	Unique_YakushimaMonster003_BatCharge = 374,
	Unique_YakushimaBoss001_Green_PhantasmalBolt = 375,
	Unique_YakushimaBoss001_Green_PhantasmalEye = 376,
	Unique_YakushimaBoss001_Green_PhantasmalSphere = 377,
	Unique_YakushimaBoss001_Green_PhantasmalDeathray = 378,
	Unique_YakushimaBoss002_PhantasmalBolt = 379,
	Unique_YakushimaBoss002_PhantasmalEye = 380,
	Unique_YakushimaBoss002_PhantasmalSphere = 381,
	Unique_YakushimaBoss002_PhantasmalDeathray = 382,
	Unique_YakushimaBoss001_Green_2_PhantasmalBolt = 383,
	Unique_YakushimaBoss001_Green_2_PhantasmalEye = 384,
	Unique_YakushimaBoss001_Green_2_PhantasmalSphere = 385,
	Unique_YakushimaBoss001_Green_2_PhantasmalDeathray = 386,
	Unique_YakushimaBoss002_2_PhantasmalBolt = 387,
	Unique_YakushimaBoss002_2_PhantasmalEye = 388,
	Unique_YakushimaBoss002_2_PhantasmalSphere = 389,
	Unique_YakushimaBoss002_2_PhantasmalDeathray = 390,
	MAX = 391,
}

/**
 * Convert from game's enum format (e.g., 'EPalWazaID::266' or 'EPalWazaID::Unique_LegendDeer_RadiantWingRush').
 *
 * Returns a tuple of [WazaID | null, skillIdString].
 * The skillIdString is always prefixed with 'EPalWazaID::' for consistency.
 */
export function wazaIdFromStr(value: string): [WazaID | null, string] {
	if (!value.startsWith("EPalWazaID::")) {
		return [null, value];
	}

	const idPart = value.split("::").pop()!;

	// Try to parse as a name first
	if (idPart in WazaID) {
		const waza = WazaID[idPart as keyof typeof WazaID];
		if (typeof waza === "number") {
			return [waza, `EPalWazaID::${WazaID[waza]}`];
		}
	}

	// Try to parse as a numeric value
	const numericId = parseInt(idPart, 10);
	if (!isNaN(numericId) && numericId in WazaID) {
		return [numericId as WazaID, `EPalWazaID::${WazaID[numericId]}`];
	}

	// Both lookups failed
	return [null, value];
}

/**
 * Convert a WazaID back to game's enum format with name.
 */
export function wazaIdToStr(waza: WazaID): string {
	return `EPalWazaID::${WazaID[waza]}`;
}

export const suitabilityImageMap = {
	EmitFlame: 'kindling',
	Watering: 'watering',
	Seeding: 'planting',
	GenerateElectricity: 'generating',
	Handcraft: 'handiwork',
	Collection: 'gathering',
	Deforest: 'deforesting',
	Mining: 'mining',
	OilExtraction: 'extracting',
	ProductMedicine: 'production',
	Cool: 'cooling',
	Transport: 'transporting',
	MonsterFarm: 'farming'
};

export function getWorkSuitabilityFormattedName(suitability: WorkSuitability): string {
	return (
		suitabilityImageMap[suitability].charAt(0).toUpperCase() +
		suitabilityImageMap[suitability].slice(1)
	);
}