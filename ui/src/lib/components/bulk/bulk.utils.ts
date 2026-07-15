import type { Guild, GuildSummary, Pal, Player, PlayerSummary } from '$types';

export interface PlayerRow {
	uid: string;
	nickname: string;
	level: number | null;
	guildName: string;
	pal_count: number;
	lastOnline: string | null;
}

export interface GuildRow {
	id: string;
	name: string;
	player_count: number;
	pal_count: number;
	level: number | null;
	base_count: number;
}

const UNKNOWN = '—';

export function buildPlayerRows(players: PlayerSummary[], guilds: GuildSummary[]): PlayerRow[] {
	const guildNameById = new Map(guilds.map((guild) => [guild.id, guild.name]));
	return players.map((player) => ({
		uid: player.uid,
		nickname: player.nickname,
		level: player.level ?? null,
		guildName: (player.guild_id && guildNameById.get(player.guild_id)) || UNKNOWN,
		pal_count: player.pal_count,
		lastOnline: player.last_online_time ?? null
	}));
}

export function buildGuildRows(guilds: GuildSummary[]): GuildRow[] {
	return guilds.map((guild) => ({
		id: guild.id,
		name: guild.name,
		player_count: guild.player_count,
		pal_count: guild.pal_count,
		level: guild.level ?? null,
		base_count: guild.base_count
	}));
}

export function filterBySearch<T>(rows: T[], query: string, fields: (keyof T)[]): T[] {
	const needle = query.trim().toLowerCase();
	if (!needle) return rows;
	return rows.filter((row) =>
		fields.some((field) => String(row[field] ?? '').toLowerCase().includes(needle))
	);
}

export function daysSince(iso: string | null, nowMs: number): number | null {
	if (!iso) return null;
	const then = Date.parse(iso);
	if (Number.isNaN(then)) return null;
	return Math.floor((nowMs - then) / 86_400_000);
}

export function inactivePlayerUids(rows: PlayerRow[], minDays: number, nowMs: number): string[] {
	return rows
		.filter((row) => {
			const days = daysSince(row.lastOnline, nowMs);
			return days === null || days >= minDays;
		})
		.map((row) => row.uid);
}

export function emptyGuildIds(rows: GuildRow[]): string[] {
	return rows.filter((row) => row.player_count === 0).map((row) => row.id);
}

export function resolveBulkPal(
	player: Player | undefined,
	guild: Guild | undefined,
	palId: string | null
): Pal | undefined {
	if (!palId) return undefined;
	const fromPlayer = player?.pals?.[palId];
	if (fromPlayer) return fromPlayer;
	const bases = guild?.bases ?? {};
	for (const base of Object.values(bases)) {
		const fromBase = base?.pals?.[palId];
		if (fromBase) return fromBase;
	}
	return undefined;
}
