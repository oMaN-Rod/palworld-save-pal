import { describe, expect, it } from 'vitest';
import type { GuildSummary, PalSummary, PlayerSummary } from '$types';
import {
	buildGuildRows,
	buildPlayerRows,
	daysSince,
	emptyGuildIds,
	filterBySearch,
	groupPalIds,
	inactivePlayerUids,
	resolveBulkPal
} from './bulk.utils';

const guilds: GuildSummary[] = [
	{ id: 'g1', name: 'Alpha', player_count: 2, base_count: 1, level: 3, pal_count: 5, loaded: false },
	{ id: 'g2', name: 'Empty', player_count: 0, base_count: 0, level: 1, pal_count: 0, loaded: false }
];

const players: PlayerSummary[] = [
	{ uid: 'p1', nickname: 'Aria', level: 40, guild_id: 'g1', pal_count: 10, last_online_time: '2026-06-01T00:00:00', loaded: false },
	{ uid: 'p2', nickname: 'Bolt', level: 5, guild_id: 'gX', pal_count: 0, loaded: false }
];

describe('buildPlayerRows', () => {
	it('resolves guild name, falling back to a dash when unknown', () => {
		const rows = buildPlayerRows(players, guilds);
		expect(rows[0]).toMatchObject({ uid: 'p1', guildName: 'Alpha', level: 40, lastOnline: '2026-06-01T00:00:00' });
		expect(rows[1]).toMatchObject({ uid: 'p2', guildName: '—', level: 5, lastOnline: null });
	});
});

describe('buildGuildRows', () => {
	it('maps summary fields with null level fallback', () => {
		const rows = buildGuildRows(guilds);
		expect(rows[0]).toMatchObject({ id: 'g1', name: 'Alpha', player_count: 2, pal_count: 5, level: 3, base_count: 1 });
	});
});

describe('filterBySearch', () => {
	const rows = buildPlayerRows(players, guilds);
	it('matches case-insensitive substrings across fields', () => {
		expect(filterBySearch(rows, 'ar', ['nickname']).map((r) => r.uid)).toEqual(['p1']);
		expect(filterBySearch(rows, 'alpha', ['guildName']).map((r) => r.uid)).toEqual(['p1']);
	});
	it('returns all rows for an empty query', () => {
		expect(filterBySearch(rows, '', ['nickname'])).toHaveLength(2);
	});
});

describe('daysSince', () => {
	it('returns whole days between an ISO time and now', () => {
		const now = Date.parse('2026-06-11T00:00:00');
		expect(daysSince('2026-06-01T00:00:00', now)).toBe(10);
	});
	it('returns null for a null timestamp', () => {
		expect(daysSince(null, Date.parse('2026-06-11T00:00:00'))).toBeNull();
	});
});

describe('inactivePlayerUids', () => {
	it('selects players inactive for at least minDays; never-online counts as inactive', () => {
		const now = Date.parse('2026-06-11T00:00:00');
		const rows = buildPlayerRows(players, guilds);
		// p1 last online 10 days ago, p2 never online
		expect(inactivePlayerUids(rows, 10, now).sort()).toEqual(['p1', 'p2']);
		expect(inactivePlayerUids(rows, 11, now)).toEqual(['p2']);
	});
});

describe('emptyGuildIds', () => {
	it('selects guilds with zero members', () => {
		expect(emptyGuildIds(buildGuildRows(guilds))).toEqual(['g2']);
	});
});

describe('groupPalIds', () => {
	// A base worker reaches the frontend with no owner_uid at all: the backend filters
	// the nil guid its own writer produces, so `w1` here is the real wire shape.
	const rows = [
		{ instance_id: 'p1', owner_uid: 'owner-1' },
		{ instance_id: 'w1', guild_id: 'g1', base_id: 'b1' }
	] as PalSummary[];

	it('groups a player-owned pal under its owner', () => {
		const { byOwner, byBase } = groupPalIds(rows, ['p1']);
		expect(byOwner.get('owner-1')).toEqual(['p1']);
		expect(byBase.size).toBe(0);
	});

	it('groups an ownerless base worker under its base, not its owner', () => {
		const { byOwner, byBase } = groupPalIds(rows, ['w1']);
		expect(byOwner.size).toBe(0);
		expect(byBase.get('g1:b1')).toEqual({ guildId: 'g1', baseId: 'b1', palIds: ['w1'] });
	});

	it('ignores an id absent from the given rows', () => {
		const { byOwner, byBase } = groupPalIds(rows, ['missing']);
		expect(byOwner.size).toBe(0);
		expect(byBase.size).toBe(0);
	});
});

describe('resolveBulkPal', () => {
	const pal = { instance_id: 'x1' } as never;
	it('finds a player-owned pal by id', () => {
		const player = { pals: { x1: pal } } as never;
		expect(resolveBulkPal(player, undefined, 'x1')).toBe(pal);
	});
	it('finds a guild/base pal by id', () => {
		const guild = { bases: { b1: { pals: { x1: pal } } } } as never;
		expect(resolveBulkPal(undefined, guild, 'x1')).toBe(pal);
	});
	it('returns undefined when the id is null or absent', () => {
		expect(resolveBulkPal(undefined, undefined, 'x1')).toBeUndefined();
		expect(resolveBulkPal({ pals: {} } as never, undefined, null)).toBeUndefined();
	});
});
