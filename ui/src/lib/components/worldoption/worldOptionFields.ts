// Labels are hardcoded English on purpose: they mirror Palworld's own ini keys,
// which is what users search for. This matches the sibling envGroups.ts and is a
// deliberate departure from the app's Paraglide i18n. See the spec, decision 5.

// Must stay in sync with WoKind::wire_tag() in psp-core/src/domain/world_option.rs.
export type WoFieldKind =
	| 'bool'
	| 'int'
	| 'float'
	| 'str'
	| 'name'
	| 'enum'
	| 'enum_array'
	| 'name_array';

export type WoTab = 'general' | 'gameplay' | 'advanced';

export type WoField = {
	key: string;
	label: string;
	kind: WoFieldKind;
	/** Shown when the save omits this key. Also the value written if the user edits it. */
	default: boolean | number | string | string[];
	min?: number;
	max?: number;
	step?: number;
	/** Fully-qualified enum variants. Required for kind 'enum'. */
	options?: { value: string; label: string }[];
};

export type WoGroup = { title: string; tab: WoTab; keys: WoField[] };

// Enum options are FULLY QUALIFIED: the backend rejects bare variants.
export const DIFFICULTY_OPTIONS = [
	{ value: 'EPalOptionWorldDifficulty::None', label: 'None' },
	{ value: 'EPalOptionWorldDifficulty::Casual', label: 'Casual' },
	{ value: 'EPalOptionWorldDifficulty::Normal', label: 'Normal' },
	{ value: 'EPalOptionWorldDifficulty::Hard', label: 'Hard' },
	{ value: 'EPalOptionWorldDifficulty::Custom', label: 'Custom' }
];

export const DEATH_PENALTY_OPTIONS = [
	{ value: 'EPalOptionWorldDeathPenalty::None', label: 'None' },
	{ value: 'EPalOptionWorldDeathPenalty::Item', label: 'Drop items' },
	{ value: 'EPalOptionWorldDeathPenalty::ItemAndEquipment', label: 'Drop items & equipment' },
	{ value: 'EPalOptionWorldDeathPenalty::All', label: 'Drop all, including Pals' }
];

export const RANDOMIZER_OPTIONS = [
	{ value: 'EPalRandomizerType::None', label: 'None' },
	{ value: 'EPalRandomizerType::Region', label: 'Region' },
	{ value: 'EPalRandomizerType::All', label: 'All' }
];

export const LOG_FORMAT_OPTIONS = [
	{ value: 'EPalLogFormatType::Text', label: 'Text' },
	{ value: 'EPalLogFormatType::Json', label: 'JSON' }
];

export const CROSSPLAY_PLATFORMS = [
	{ value: 'EPalAllowConnectPlatform::Steam', label: 'Steam' },
	{ value: 'EPalAllowConnectPlatform::Xbox', label: 'Xbox' },
	{ value: 'EPalAllowConnectPlatform::PS5', label: 'PS5' },
	{ value: 'EPalAllowConnectPlatform::Mac', label: 'Mac' }
];

const rate = (key: string, label: string, def: number): WoField => ({
	key,
	label,
	kind: 'float',
	default: def,
	min: 0,
	max: 100,
	step: 0.1
});
// Engine-scale floats (distances in cm, intervals in seconds) whose values run far
// above a rate multiplier. No upper cap: there is no sensible universal bound, and a
// low `max` would clamp a legitimate value like 15000 down on edit.
const bigFloat = (key: string, label: string, def: number): WoField => ({
	key,
	label,
	kind: 'float',
	default: def,
	min: 0,
	step: 1
});
const bool = (key: string, label: string, def: boolean): WoField => ({
	key,
	label,
	kind: 'bool',
	default: def
});
const int = (key: string, label: string, def: number, min = 0, max = 2147483647): WoField => ({
	key,
	label,
	kind: 'int',
	default: def,
	min,
	max
});
const text = (key: string, label: string, def = ''): WoField => ({
	key,
	label,
	kind: 'str',
	default: def
});

export const worldOptionGroups: WoGroup[] = [
	{
		title: 'Server Identity',
		tab: 'general',
		keys: [
			text('ServerName', 'Server Name', 'Default Palworld Server'),
			text('ServerDescription', 'Server Description'),
			text('ServerPassword', 'Server Password'),
			text('AdminPassword', 'Admin Password'),
			text('PublicIP', 'Public IP'),
			text('Region', 'Region'),
			int('PublicPort', 'Public Port', 8211, 1, 65535),
			int('ServerPlayerMaxNum', 'Max Players (server)', 32, 1, 32),
			int('CoopPlayerMaxNum', 'Max Players (co-op)', 4, 1, 32),
			bool('bIsMultiplay', 'Multiplayer', false),
			bool('bShowPlayerList', 'Show Player List', false)
		]
	},
	{
		title: 'Network & API',
		tab: 'general',
		keys: [
			bool('RCONEnabled', 'RCON Enabled', false),
			int('RCONPort', 'RCON Port', 25575, 1, 65535),
			bool('RESTAPIEnabled', 'REST API Enabled', false),
			int('RESTAPIPort', 'REST API Port', 8212, 1, 65535),
			bool('bUseAuth', 'Use Auth', true),
			text('BanListURL', 'Ban List URL', 'https://api.palworldgame.com/api/banlist.txt'),
			{
				key: 'CrossplayPlatforms',
				label: 'Crossplay Platforms',
				kind: 'enum_array',
				default: CROSSPLAY_PLATFORMS.map((p) => p.value)
			},
			bool('bAllowClientMod', 'Allow Client Mods', true),
			int('ChatPostLimitPerMinute', 'Chat Posts / Minute', 30),
			{
				key: 'LogFormatType',
				label: 'Log Format',
				kind: 'enum',
				default: 'EPalLogFormatType::Text',
				options: LOG_FORMAT_OPTIONS
			},
			bool('bIsShowJoinLeftMessage', 'Show Join/Leave Messages', true),
			bool('bIsUseBackupSaveData', 'Use Backup Save Data', true)
		]
	},
	{
		title: 'Difficulty & Randomizer',
		tab: 'gameplay',
		keys: [
			{
				key: 'Difficulty',
				label: 'Difficulty',
				kind: 'enum',
				default: 'EPalOptionWorldDifficulty::None',
				options: DIFFICULTY_OPTIONS
			},
			{
				key: 'DeathPenalty',
				label: 'Death Penalty',
				kind: 'enum',
				default: 'EPalOptionWorldDeathPenalty::All',
				options: DEATH_PENALTY_OPTIONS
			},
			{
				key: 'RandomizerType',
				label: 'Randomizer Type',
				kind: 'enum',
				default: 'EPalRandomizerType::None',
				options: RANDOMIZER_OPTIONS
			},
			text('RandomizerSeed', 'Randomizer Seed'),
			bool('bIsRandomizerPalLevelRandom', 'Randomize Pal Levels', false)
		]
	},
	{
		title: 'Time & Rates',
		tab: 'gameplay',
		keys: [
			rate('DayTimeSpeedRate', 'Day Speed Rate', 1),
			rate('NightTimeSpeedRate', 'Night Speed Rate', 1),
			rate('ExpRate', 'EXP Rate', 1),
			rate('WorkSpeedRate', 'Work Speed Rate', 1),
			bigFloat('autoSaveSpan', 'Auto Save Span (s)', 30)
		]
	},
	{
		title: 'Pal Rates',
		tab: 'gameplay',
		keys: [
			rate('PalCaptureRate', 'Capture Rate', 1),
			rate('PalSpawnNumRate', 'Spawn Number Rate', 1),
			rate('PalDamageRateAttack', 'Pal Damage Dealt', 1),
			rate('PalDamageRateDefense', 'Pal Damage Taken', 1),
			rate('PalStomachDecreaceRate', 'Pal Hunger Depletion', 1),
			rate('PalStaminaDecreaceRate', 'Pal Stamina Depletion', 1),
			rate('PalAutoHPRegeneRate', 'Pal HP Regen', 1),
			rate('PalAutoHpRegeneRateInSleep', 'Pal HP Regen (sleep)', 1),
			bigFloat('PalEggDefaultHatchingTime', 'Egg Hatching Time (h)', 72),
			rate('MonsterFarmActionSpeedRate', 'Monster Farm Speed', 1),
			bool('EnablePredatorBossPal', 'Enable Predator Boss Pals', true)
		]
	},
	{
		title: 'Player Rates',
		tab: 'gameplay',
		keys: [
			rate('PlayerDamageRateAttack', 'Player Damage Dealt', 1),
			rate('PlayerDamageRateDefense', 'Player Damage Taken', 1),
			rate('PlayerStomachDecreaceRate', 'Player Hunger Depletion', 1),
			rate('PlayerStaminaDecreaceRate', 'Player Stamina Depletion', 1),
			rate('PlayerAutoHPRegeneRate', 'Player HP Regen', 1),
			rate('PlayerAutoHpRegeneRateInSleep', 'Player HP Regen (sleep)', 1)
		]
	},
	{
		title: 'Stat Enhancement',
		tab: 'gameplay',
		keys: [
			bool('bAllowEnhanceStat_Health', 'Allow Health Enhancement', true),
			bool('bAllowEnhanceStat_Attack', 'Allow Attack Enhancement', true),
			bool('bAllowEnhanceStat_Stamina', 'Allow Stamina Enhancement', true),
			bool('bAllowEnhanceStat_Weight', 'Allow Weight Enhancement', true),
			bool('bAllowEnhanceStat_WorkSpeed', 'Allow Work Speed Enhancement', true)
		]
	},
	{
		title: 'Building & Objects',
		tab: 'gameplay',
		keys: [
			rate('BuildObjectHpRate', 'Build Object HP Rate', 1),
			rate('BuildObjectDamageRate', 'Build Object Damage Rate', 1),
			rate('BuildObjectDeteriorationDamageRate', 'Deterioration Damage Rate', 1),
			int('MaxBuildingLimitNum', 'Max Building Limit (0 = none)', 0),
			bool('bBuildAreaLimit', 'Build Area Limit', false),
			bool('bEnableBuildingPlayerUIdDisplay', 'Show Builder UId', false),
			int('BuildingNameDisplayCacheTTLSeconds', 'Builder Name Cache TTL (s)', 60)
		]
	},
	{
		title: 'Items & Drops',
		tab: 'gameplay',
		keys: [
			rate('CollectionDropRate', 'Collection Drop Rate', 1),
			rate('CollectionObjectHpRate', 'Collection Object HP Rate', 1),
			rate('CollectionObjectRespawnSpeedRate', 'Collection Respawn Speed', 1),
			rate('EnemyDropItemRate', 'Enemy Drop Rate', 1),
			int('DropItemMaxNum', 'Max Dropped Items', 3000),
			int('DropItemMaxNum_UNKO', 'Max Dropped UNKO', 100),
			int('PhysicsActiveDropItemMaxNum', 'Max Physics Drops (-1 = unlimited)', -1, -1),
			rate('DropItemAliveMaxHours', 'Drop Lifetime (h)', 1),
			rate('ItemWeightRate', 'Item Weight Rate', 1),
			rate('EquipmentDurabilityDamageRate', 'Equipment Durability Damage', 1),
			rate('ItemCorruptionMultiplier', 'Item Corruption Multiplier', 1),
			bool('bActiveUNKO', 'Active UNKO', false),
			int('SupplyDropSpan', 'Supply Drop Span (min)', 180)
		]
	},
	{
		title: 'Guild & Base',
		tab: 'gameplay',
		keys: [
			int('GuildPlayerMaxNum', 'Max Guild Players', 20),
			int('BaseCampMaxNum', 'Max Base Camps', 128),
			int('BaseCampMaxNumInGuild', 'Max Base Camps / Guild', 4),
			int('BaseCampWorkerMaxNum', 'Max Base Workers', 15),
			bool('bAutoResetGuildNoOnlinePlayers', 'Auto Reset Inactive Guilds', false),
			bigFloat('AutoResetGuildTimeNoOnlinePlayers', 'Auto Reset After (h)', 72),
			int('GuildRejoinCooldownMinutes', 'Guild Rejoin Cooldown (min)', 0),
			int('MaxGuildsPerFrame', 'Max Guilds Per Frame', 10)
		]
	},
	{
		title: 'PvP & Hardcore',
		tab: 'gameplay',
		keys: [
			bool('bIsPvP', 'PvP', false),
			bool('bEnablePlayerToPlayerDamage', 'Player-to-Player Damage', false),
			bool('bEnableFriendlyFire', 'Friendly Fire', false),
			bool('bHardcore', 'Hardcore', false),
			bool('bPalLost', 'Pals Lost on Death', false),
			bool('bCharacterRecreateInHardcore', 'Recreate Character in Hardcore', false),
			bool('bCanPickupOtherGuildDeathPenaltyDrop', 'Pick Up Other Guild Drops', false),
			bool('bEnableDefenseOtherGuildPlayer', 'Defend Against Other Guilds', false),
			bool('bDisplayPvPItemNumOnWorldMap_BaseCamp', 'Show PvP Items on Map (Base)', false),
			bool('bDisplayPvPItemNumOnWorldMap_Player', 'Show PvP Items on Map (Player)', false),
			bool('bAdditionalDropItemWhenPlayerKillingInPvPMode', 'Extra Drop on PvP Kill', false),
			{
				key: 'AdditionalDropItemWhenPlayerKillingInPvPMode',
				label: 'Extra Drop Item Id',
				kind: 'name',
				default: 'PlayerDropItem'
			},
			int('AdditionalDropItemNumWhenPlayerKillingInPvPMode', 'Extra Drop Count', 1)
		]
	},
	{
		title: 'Respawn',
		tab: 'gameplay',
		keys: [
			rate('BlockRespawnTime', 'Block Respawn Time (s)', 5),
			rate('RespawnPenaltyDurationThreshold', 'Respawn Penalty Threshold', 0),
			rate('RespawnPenaltyTimeScale', 'Respawn Penalty Time Scale', 2)
		]
	},
	{
		title: 'World & Travel',
		tab: 'gameplay',
		keys: [
			bool('bEnableInvaderEnemy', 'Enable Invaders', true),
			bool('bEnableFastTravel', 'Enable Fast Travel', true),
			bool('bEnableFastTravelOnlyBaseCamp', 'Fast Travel Only to Base Camps', false),
			bool('bIsStartLocationSelectByMap', 'Select Start Location on Map', true),
			bool('bExistPlayerAfterLogout', 'Player Persists After Logout', false),
			bool('bEnableNonLoginPenalty', 'Non-Login Penalty', true),
			bool('bInvisibleOtherGuildBaseCampAreaFX', 'Hide Other Guild Base FX', false)
		]
	},
	{
		title: 'Voice Chat',
		tab: 'advanced',
		keys: [
			bool('bEnableVoiceChat', 'Enable Voice Chat', false),
			bigFloat('VoiceChatMaxVolumeDistance', 'Max Volume Distance', 3000),
			bigFloat('VoiceChatZeroVolumeDistance', 'Zero Volume Distance', 15000)
		]
	},
	{
		title: 'Global Palbox',
		tab: 'advanced',
		keys: [
			bool('bAllowGlobalPalboxExport', 'Allow Global Palbox Export', true),
			bool('bAllowGlobalPalboxImport', 'Allow Global Palbox Import', false)
		]
	},
	{
		title: 'Technology',
		tab: 'advanced',
		keys: [
			{
				key: 'DenyTechnologyList',
				label: 'Denied Technologies',
				kind: 'name_array',
				default: []
			}
		]
	},
	{
		title: 'Performance',
		tab: 'advanced',
		keys: [
			bigFloat('ServerReplicatePawnCullDistance', 'Pawn Cull Distance', 15000),
			rate('ItemContainerForceMarkDirtyInterval', 'Container Mark-Dirty Interval', 1),
			rate('PlayerDataPalStorageUpdateCheckTickInterval', 'Pal Storage Check Interval', 1),
			bigFloat('AutoTransferMasterCheckIntervalSeconds', 'Master Transfer Check (s)', 3600),
			int('AutoTransferMasterThresholdDays', 'Master Transfer Threshold (days)', 14)
		]
	},
	{
		title: 'Input',
		tab: 'advanced',
		keys: [
			bool('bEnableAimAssistPad', 'Aim Assist (Gamepad)', true),
			bool('bEnableAimAssistKeyboard', 'Aim Assist (Keyboard)', false)
		]
	}
];

export const worldOptionTabs: { id: WoTab; label: string }[] = [
	{ id: 'general', label: 'General' },
	{ id: 'gameplay', label: 'Gameplay' },
	{ id: 'advanced', label: 'Advanced' }
];

/** Groups belonging to a tab. Filters on the group's own `tab`, never on a title
 *  list -- CreateServerModal filters by title and silently drops a mistyped group. */
export function groupsForTab(tab: WoTab): WoGroup[] {
	return worldOptionGroups.filter((group) => group.tab === tab);
}

const fieldIndex = new Map<string, WoField>(
	worldOptionGroups.flatMap((group) => group.keys.map((field) => [field.key, field] as const))
);

export function fieldFor(key: string): WoField | undefined {
	return fieldIndex.get(key);
}

export const allWorldOptionFields: WoField[] = [...fieldIndex.values()];
