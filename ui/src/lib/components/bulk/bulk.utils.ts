import type { Guild, GuildSummary, Pal, PalSummary, Player, PlayerSummary } from '$types';

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

// A guild-base worker pal carries this as its owner_uid, not an absent field.
const NIL_OWNER_UID = '00000000-0000-0000-0000-000000000000';

export interface PalIdGroups {
	byOwner: Map<string, string[]>;
	byBase: Map<string, { guildId: string; baseId: string; palIds: string[] }>;
}

export function groupPalIds(rows: PalSummary[], ids: string[]): PalIdGroups {
	const rowById = new Map(rows.map((row) => [row.instance_id, row]));
	const byOwner = new Map<string, string[]>();
	const byBase = new Map<string, { guildId: string; baseId: string; palIds: string[] }>();
	for (const id of ids) {
		const row = rowById.get(id);
		if (!row) continue;
		if (row.owner_uid && row.owner_uid !== NIL_OWNER_UID) {
			const group = byOwner.get(row.owner_uid) ?? [];
			group.push(id);
			byOwner.set(row.owner_uid, group);
		} else if (row.guild_id && row.base_id) {
			const key = `${row.guild_id}:${row.base_id}`;
			const group = byBase.get(key) ?? { guildId: row.guild_id, baseId: row.base_id, palIds: [] };
			group.palIds.push(id);
			byBase.set(key, group);
		}
	}
	return { byOwner, byBase };
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
