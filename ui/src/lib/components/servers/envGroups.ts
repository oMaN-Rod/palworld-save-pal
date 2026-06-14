export type EnvFieldType = 'text' | 'bool';
export type EnvKey = { key: string; label: string; default: string; type?: EnvFieldType };
export type EnvGroup = { title: string; keys: EnvKey[] };

/** Official Palworld server settings documentation */
export const PALWORLD_DOCS_URL = 'https://docs.palworldgame.com/category/settings-and-operations';

/** Helper: marks a key as boolean */
const bool = (key: string, label: string, defaultVal: string): EnvKey => ({
	key,
	label,
	default: defaultVal,
	type: 'bool'
});

export const envGroups: EnvGroup[] = [
	{
		title: 'Gameplay Rates',
		keys: [
			{ key: 'EXP_RATE', label: 'EXP Rate', default: '1.000000' },
			{ key: 'PAL_CAPTURE_RATE', label: 'Pal Capture Rate', default: '1.000000' },
			{ key: 'PAL_SPAWN_NUM_RATE', label: 'Pal Spawn Rate', default: '1.000000' },
			{ key: 'PAL_DAMAGE_RATE_ATTACK', label: 'Pal Attack Damage', default: '1.000000' },
			{ key: 'PAL_DAMAGE_RATE_DEFENSE', label: 'Pal Defense Rate', default: '1.000000' },
			{ key: 'PLAYER_DAMAGE_RATE_ATTACK', label: 'Player Attack Damage', default: '1.000000' },
			{ key: 'PLAYER_DAMAGE_RATE_DEFENSE', label: 'Player Defense Rate', default: '1.000000' },
			{ key: 'PAL_STOMACH_DECREASE_RATE', label: 'Pal Hunger Rate', default: '1.000000' },
			{ key: 'PAL_STAMINA_DECREASE_RATE', label: 'Pal Stamina Rate', default: '1.000000' },
			{ key: 'PAL_AUTO_HP_REGEN_RATE', label: 'Pal HP Regen Rate', default: '1.000000' },
			{
				key: 'PAL_AUTO_HP_REGEN_RATE_IN_SLEEP',
				label: 'Pal HP Regen Sleep',
				default: '1.000000'
			},
			{ key: 'PLAYER_STOMACH_DECREASE_RATE', label: 'Player Hunger Rate', default: '1.000000' },
			{
				key: 'PLAYER_STAMINA_DECREASE_RATE',
				label: 'Player Stamina Rate',
				default: '1.000000'
			},
			{
				key: 'PLAYER_AUTO_HP_REGEN_RATE',
				label: 'Player HP Regen Rate',
				default: '1.000000'
			},
			{
				key: 'PLAYER_AUTO_HP_REGEN_RATE_IN_SLEEP',
				label: 'HP Regen Sleep Rate',
				default: '1.000000'
			},
			{ key: 'COLLECTION_DROP_RATE', label: 'Collection Drop Rate', default: '1.000000' },
			{ key: 'COLLECTION_OBJECT_HP_RATE', label: 'Collection Object HP', default: '1.000000' },
			{
				key: 'COLLECTION_OBJECT_RESPAWN_SPEED_RATE',
				label: 'Respawn Speed',
				default: '1.000000'
			},
			{ key: 'ENEMY_DROP_ITEM_RATE', label: 'Enemy Drop Rate', default: '1.000000' },
			{ key: 'WORK_SPEED_RATE', label: 'Work Speed Rate', default: '1.000000' },
			{ key: 'ITEM_WEIGHT_RATE', label: 'Item Weight Rate', default: '1.000000' },
			{
				key: 'EQUIPMENT_DURABILITY_DAMAGE_RATE',
				label: 'Equipment Durability',
				default: '1.000000'
			},
			{
				key: 'ITEM_CORRUPTION_MULTIPLIER',
				label: 'Item Corruption Rate',
				default: '1.000000'
			}
		]
	},
	{
		title: 'Time & Difficulty',
		keys: [
			{ key: 'DIFFICULTY', label: 'Difficulty', default: 'None' },
			{ key: 'DAYTIME_SPEEDRATE', label: 'Daytime Speed', default: '1.000000' },
			{ key: 'NIGHTTIME_SPEEDRATE', label: 'Nighttime Speed', default: '1.000000' },
			{
				key: 'PAL_EGG_DEFAULT_HATCHING_TIME',
				label: 'Egg Hatch Time (hrs)',
				default: '72.000000'
			},
			{ key: 'AUTO_SAVE_SPAN', label: 'Auto Save Interval (min)', default: '30.000000' },
			{
				key: 'DROP_ITEM_ALIVE_MAX_HOURS',
				label: 'Drop Item Lifetime (hrs)',
				default: '1.000000'
			},
			{ key: 'SUPPLY_DROP_SPAN', label: 'Supply Drop Interval (min)', default: '180' }
		]
	},
	{
		title: 'Server Settings',
		keys: [
			bool('MULTITHREADING', 'Multithreading', 'true'),
			bool('COMMUNITY', 'Public Server', 'false'),
			bool('UPDATE_ON_BOOT', 'Update on Boot', 'true'),
			{ key: 'PUBLIC_IP', label: 'Public IP', default: '' },
			{ key: 'PUBLIC_PORT', label: 'Public Port', default: '' },
			{ key: 'REGION', label: 'Region', default: '' },
			bool('USEAUTH', 'Use Auth', 'True'),
			bool('SHOW_PLAYER_LIST', 'Show Player List', 'True'),
			bool('SHOW_JOIN_LEFT_MESSAGE', 'Show Join/Leave Messages', 'True'),
			bool('ALLOW_CLIENT_MOD', 'Allow Modded Clients', 'True'),
			{ key: 'CHAT_POST_LIMIT_PER_MINUTE', label: 'Chat Rate Limit (/min)', default: '10' },
			{
				key: 'BAN_LIST_URL',
				label: 'Ban List URL',
				default: 'https://api.palworldgame.com/api/banlist.txt'
			},
			{
				key: 'CROSSPLAY_PLATFORMS',
				label: 'Crossplay Platforms',
				default: '(Steam,Xbox,PS5,Mac)'
			}
		]
	},
	{
		title: 'Stat Enhancement',
		keys: [
			bool('ALLOW_ENHANCE_STAT_HEALTH', 'Allow HP Stat Points', 'True'),
			bool('ALLOW_ENHANCE_STAT_ATTACK', 'Allow Attack Stat Points', 'True'),
			bool('ALLOW_ENHANCE_STAT_STAMINA', 'Allow Stamina Stat Points', 'True'),
			bool('ALLOW_ENHANCE_STAT_WEIGHT', 'Allow Carry Weight Stat Points', 'True'),
			bool('ALLOW_ENHANCE_STAT_WORK_SPEED', 'Allow Work Speed Stat Points', 'True')
		]
	},
	{
		title: 'PvP / Hardcore',
		keys: [
			bool('IS_PVP', 'PvP Mode', 'False'),
			bool('ENABLE_PLAYER_TO_PLAYER_DAMAGE', 'Player vs Player Damage', 'False'),
			bool('ENABLE_FRIENDLY_FIRE', 'Friendly Fire', 'False'),
			bool('ENABLE_DEFENSE_OTHER_GUILD_PLAYER', 'Defense Other Guild', 'False'),
			bool('HARDCORE', 'Hardcore Mode', 'False'),
			bool('CHARACTER_RECREATE_IN_HARDCORE', 'Recreate Character in Hardcore', 'False'),
			bool('PAL_LOST', 'Pal Lost on Death', 'False'),
			{ key: 'DEATH_PENALTY', label: 'Death Penalty', default: 'All' },
			bool('CAN_PICKUP_OTHER_GUILD_DEATH_PENALTY_DROP', 'Pickup Other Guild Drops', 'False'),
			bool('ENABLE_AIM_ASSIST_PAD', 'Aim Assist (Pad)', 'True'),
			bool('ENABLE_AIM_ASSIST_KEYBOARD', 'Aim Assist (KB)', 'False'),
			bool('ENABLE_INVADER_ENEMY', 'Enable Invaders', 'True'),
			bool('ENABLE_PREDATOR_BOSS_PAL', 'Predator Boss Pals', 'True'),
			bool('ENABLE_NON_LOGIN_PENALTY', 'Non-Login Penalty', 'True'),
			bool('ENABLE_FAST_TRAVEL', 'Fast Travel', 'True'),
			bool('ENABLE_FAST_TRAVEL_ONLY_BASE_CAMP', 'Fast Travel Base Camp Only', 'False'),
			bool('EXIST_PLAYER_AFTER_LOGOUT', 'Player Exists After Logout', 'False'),
			bool('IS_START_LOCATION_SELECT_BY_MAP', 'Map Start Location', 'True')
		]
	},
	{
		title: 'PvP Respawn & Rewards',
		keys: [
			{ key: 'BLOCK_RESPAWN_TIME', label: 'Respawn Delay (sec)', default: '5.0' },
			{
				key: 'RESPAWN_PENALTY_DURATION_THRESHOLD',
				label: 'Respawn Penalty Threshold (sec)',
				default: '1800.0'
			},
			{
				key: 'RESPAWN_PENALTY_TIME_SCALE',
				label: 'Respawn Penalty Scale',
				default: '2.0'
			},
			bool(
				'ADDITIONAL_DROP_ITEM_WHEN_PLAYER_KILLING_IN_PVP',
				'PvP Kill Drop Item',
				'False'
			),
			{
				key: 'ADDITIONAL_DROP_ITEM_PVP_ITEM',
				label: 'PvP Kill Drop Item ID',
				default: ''
			},
			{ key: 'ADDITIONAL_DROP_ITEM_PVP_NUM', label: 'PvP Kill Drop Qty', default: '1' },
			bool(
				'DISPLAY_PVP_ITEM_NUM_ON_WORLDMAP_BASECAMP',
				'Show PvP Items on Map (Base)',
				'False'
			),
			bool(
				'DISPLAY_PVP_ITEM_NUM_ON_WORLDMAP_PLAYER',
				'Show PvP Items on Map (Player)',
				'False'
			)
		]
	},
	{
		title: 'Guild / Building',
		keys: [
			{ key: 'GUILD_PLAYER_MAX_NUM', label: 'Guild Max Players', default: '20' },
			{
				key: 'GUILD_REJOIN_COOLDOWN_MINUTES',
				label: 'Guild Rejoin Cooldown (min)',
				default: '60'
			},
			{ key: 'BASE_CAMP_MAX_NUM', label: 'Max Base Camps', default: '128' },
			{ key: 'BASE_CAMP_MAX_NUM_IN_GUILD', label: 'Max Camps Per Guild', default: '4' },
			{ key: 'BASE_CAMP_WORKER_MAX_NUM', label: 'Max Base Workers', default: '15' },
			{ key: 'BUILD_OBJECT_HP_RATE', label: 'Build Object HP Rate', default: '1.000000' },
			{ key: 'BUILD_OBJECT_DAMAGE_RATE', label: 'Build Damage Rate', default: '1.000000' },
			{
				key: 'BUILD_OBJECT_DETERIORATION_DAMAGE_RATE',
				label: 'Deterioration Rate',
				default: '1.000000'
			},
			bool('BUILD_AREA_LIMIT', 'Build Area Limit', 'False'),
			{ key: 'MAX_BUILDING_LIMIT_NUM', label: 'Max Buildings (0=unlimited)', default: '0' },
			bool('AUTO_RESET_GUILD_NO_ONLINE_PLAYERS', 'Auto Reset Empty Guilds', 'False'),
			{
				key: 'AUTO_RESET_GUILD_TIME_NO_ONLINE_PLAYERS',
				label: 'Reset Time (hrs)',
				default: '72.000000'
			},
			bool('INVISIBLE_OTHER_GUILD_BASE_CAMP_AREA_FX', 'Hide Other Guild FX', 'False')
		]
	},
	{
		title: 'Items & Drops',
		keys: [
			{ key: 'DROP_ITEM_MAX_NUM', label: 'Drop Item Max', default: '3000' },
			{ key: 'DROP_ITEM_MAX_NUM_UNKO', label: 'Fertilizer Max', default: '100' },
			bool('ACTIVE_UNKO', 'Active Fertilizer', 'False'),
			{ key: 'COOP_PLAYER_MAX_NUM', label: 'Co-op Max Players', default: '4' },
			bool('ALLOW_GLOBAL_PALBOX_EXPORT', 'Global Palbox Export', 'True'),
			bool('ALLOW_GLOBAL_PALBOX_IMPORT', 'Global Palbox Import', 'False'),
			bool('IS_MULTIPLAY', 'Multiplayer', 'False')
		]
	},
	{
		title: 'REST API & Logging',
		keys: [
			bool('REST_API_ENABLED', 'REST API Enabled', 'True'),
			{ key: 'REST_API_PORT', label: 'REST API Port', default: '8212' },
			bool('RCON_ENABLED', 'RCON Enabled', 'False'),
			{ key: 'RCON_PORT', label: 'RCON Port', default: '25575' },
			bool('ENABLE_PLAYER_LOGGING', 'Player Logging', 'true'),
			{ key: 'PLAYER_LOGGING_POLL_PERIOD', label: 'Logging Poll (sec)', default: '5' },
			bool('LOG_FILTER_ENABLED', 'Log Filter', 'true'),
			{ key: 'LOG_FORMAT_TYPE', label: 'Log Format', default: 'Text' }
		]
	},
	{
		title: 'Backup Settings',
		keys: [
			bool('BACKUP_ENABLED', 'Backup Enabled', 'true'),
			{ key: 'BACKUP_CRON_EXPRESSION', label: 'Backup Cron', default: '0 0 * * *' },
			bool('DELETE_OLD_BACKUPS', 'Delete Old Backups', 'false'),
			{ key: 'OLD_BACKUP_DAYS', label: 'Backup Retention Days', default: '30' },
			bool('USE_BACKUP_SAVE_DATA', 'Use Backup Save Data', 'True')
		]
	},
	{
		title: 'Auto Update / Reboot',
		keys: [
			bool('AUTO_UPDATE_ENABLED', 'Auto Update', 'false'),
			{ key: 'AUTO_UPDATE_CRON_EXPRESSION', label: 'Update Cron', default: '0 * * * *' },
			{ key: 'AUTO_UPDATE_WARN_MINUTES', label: 'Update Warning (min)', default: '30' },
			bool('AUTO_REBOOT_ENABLED', 'Auto Reboot', 'false'),
			{ key: 'AUTO_REBOOT_CRON_EXPRESSION', label: 'Reboot Cron', default: '0 0 * * *' },
			{ key: 'AUTO_REBOOT_WARN_MINUTES', label: 'Reboot Warning (min)', default: '5' },
			bool('AUTO_REBOOT_EVEN_IF_PLAYERS_ONLINE', 'Reboot With Players', 'false'),
			bool('USE_DEPOT_DOWNLOADER', 'Use Depot Downloader', 'False'),
			bool('INSTALL_BETA_INSIDER', 'Install Beta', 'False')
		]
	},
	{
		title: 'Discord Integration',
		keys: [
			{ key: 'DISCORD_WEBHOOK_URL', label: 'Webhook URL', default: '' },
			{
				key: 'DISCORD_SUPPRESS_NOTIFICATIONS',
				label: 'Suppress Notifications',
				default: ''
			},
			{ key: 'DISCORD_CONNECT_TIMEOUT', label: 'Connect Timeout', default: '30' },
			{ key: 'DISCORD_MAX_TIMEOUT', label: 'Max Timeout', default: '30' }
		]
	},
	{
		title: 'UE4SS / Mods',
		keys: [
			bool('ENABLE_UE4SS', 'Enable UE4SS', 'true'),
			{ key: 'UE4SS_VERSION', label: 'UE4SS Version', default: '3.0.1' },
			bool('UE4SS_FORCE_UPDATE', 'Force Update UE4SS', 'false')
		]
	},
	{
		title: 'Engine / Performance',
		keys: [
			{ key: 'LAN_SERVER_MAX_TICK_RATE', label: 'LAN Tick Rate', default: '120' },
			{ key: 'NET_SERVER_MAX_TICK_RATE', label: 'Net Tick Rate', default: '120' },
			bool('SMOOTH_FRAME_RATE', 'Smooth Frame Rate', 'true'),
			{
				key: 'SMOOTH_FRAME_RATE_UPPER_LIMIT',
				label: 'FPS Upper Limit',
				default: '120.000000'
			},
			{
				key: 'SMOOTH_FRAME_RATE_LOWER_LIMIT',
				label: 'FPS Lower Limit',
				default: '30.000000'
			},
			{
				key: 'SERVER_REPLICATE_PAWN_CULL_DISTANCE',
				label: 'Pawn Cull Distance',
				default: '15000.000000'
			},
			{
				key: 'ITEM_CONTAINER_FORCE_MARK_DIRTY_INTERVAL',
				label: 'Container Sync Interval',
				default: '1.000000'
			}
		]
	},
	{
		title: 'Randomizer',
		keys: [
			{ key: 'RANDOMIZER_TYPE', label: 'Randomizer Type (None/Region/All)', default: '' },
			{ key: 'RANDOMIZER_SEED', label: 'Randomizer Seed', default: 'none' },
			bool('IS_RANDOMIZER_PAL_LEVEL_RANDOM', 'Random Pal Levels', 'False')
		]
	}
];

/** Parse a string as boolean (case-insensitive true/false) */
export function isTruthy(value: string): boolean {
	return value.toLowerCase() === 'true' || value === '1';
}
