import { describe, expect, it } from 'vitest';
import type { GameInventoryContainerJson, GamePalJson } from '$states/gameState.svelte';
import {
	countResearched,
	lastOnlineDate,
	partitionContainers,
	seatPals,
	toResearchGuildShape
} from '../liveGuild.utils';

function container(type: string, containerId: string): GameInventoryContainerJson {
	return { containerId, type, slotNum: 10, status: 'ok', slots: [], buildObjectId: null, baseId: null };
}

function pal(slotIndex: number, instanceId: string): GamePalJson {
	return { characterId: 'Lamball', instanceId, ownerUid: 'p1', slotIndex } as GamePalJson;
}

describe('toResearchGuildShape', () => {
	it('maps live research rows onto the shape buildTree consumes', () => {
		const shape = toResearchGuildShape({
			currentResearchId: 'Handcraft1',
			research: [{ researchId: 'Handcraft1', workAmount: 5000, requiredWorkAmount: 50000 }]
		});

		expect(shape.lab_research_data).toEqual([{ research_id: 'Handcraft1', work_amount: 5000 }]);
	});

	it('reads a missing lab as no research rather than throwing', () => {
		expect(toResearchGuildShape(null).lab_research_data).toEqual([]);
	});

	it('drops rows with no research id and defaults a null work amount to zero', () => {
		const shape = toResearchGuildShape({
			currentResearchId: null,
			research: [
				{ researchId: null, workAmount: 100, requiredWorkAmount: 200 },
				{ researchId: 'Cool1', workAmount: null, requiredWorkAmount: 5000 }
			]
		});

		expect(shape.lab_research_data).toEqual([{ research_id: 'Cool1', work_amount: 0 }]);
	});
});

describe('partitionContainers', () => {
	it('separates the guild chest from ordinary base storage', () => {
		const chest = container('GuildChest', 'c1');
		const box = container('Common', 'c2');

		const { chest: chests, storage } = partitionContainers([chest, box]);

		expect(chests.map((c) => c.containerId)).toEqual(['c1']);
		expect(storage.map((c) => c.containerId)).toEqual(['c2']);
	});

	it('treats every container as storage when none names a guild', () => {
		const { chest, storage } = partitionContainers([container('Common', 'c1')]);

		expect(chest).toEqual([]);
		expect(storage).toHaveLength(1);
	});

	it('does not treat a type that only contains "guild" as the chest', () => {
		const { chest, storage } = partitionContainers([container('GuildStorageBox', 'c1')]);

		expect(chest).toEqual([]);
		expect(storage).toHaveLength(1);
	});

	it('survives a container whose type the mod could not read', () => {
		const { storage } = partitionContainers([
			{
				containerId: 'c1',
				type: null as unknown as string,
				slotNum: 5,
				status: 'ok',
				slots: [],
				buildObjectId: null,
				baseId: null
			}
		]);

		expect(storage).toHaveLength(1);
	});
});

describe('seatPals', () => {
	it('seats each pal at the slot it reports and leaves the gaps empty', () => {
		const seats = seatPals([pal(0, 'a'), pal(2, 'b')], 4);

		expect(seats.map((s) => s.pal?.instanceId)).toEqual(['a', undefined, 'b', undefined]);
	});

	it('grows past the slot count when a pal sits beyond it', () => {
		const seats = seatPals([pal(7, 'a')], 4);

		expect(seats).toHaveLength(8);
		expect(seats[7].pal?.instanceId).toBe('a');
	});

	it('keeps the first pal when two report the same slot', () => {
		const seats = seatPals([pal(1, 'first'), pal(1, 'second')], 2);

		expect(seats[1].pal?.instanceId).toBe('first');
	});
});

describe('lastOnlineDate', () => {
	it('converts FDateTime ticks to a date', () => {
		// 2026-09-06T00:00:00Z expressed as 100ns units since 0001-01-01.
		const ticks = (Date.UTC(2026, 8, 6) + 62_135_596_800_000) * 10_000;

		expect(lastOnlineDate(ticks)?.toISOString()).toBe('2026-09-06T00:00:00.000Z');
	});

	it('reads a null, a zero or an implausible year as unknown', () => {
		expect(lastOnlineDate(null)).toBeNull();
		expect(lastOnlineDate(0)).toBeNull();
		expect(lastOnlineDate(1_518_203_890_000)).toBeNull();
	});
});

describe('countResearched', () => {
	const research = {
		Handcraft1: { id: 'Handcraft1', details: { category: 'Handcraft', work_amount: 50000 } },
		Handcraft2: { id: 'Handcraft2', details: { category: 'Handcraft', work_amount: 200000 } },
		Cool1: { id: 'Cool1', details: { category: 'Cool', work_amount: 5000 } }
	} as never;

	it('counts only research whose work has reached its requirement', () => {
		const counts = countResearched(
			{
				currentResearchId: null,
				research: [
					{ researchId: 'Handcraft1', workAmount: 50000, requiredWorkAmount: 50000 },
					{ researchId: 'Handcraft2', workAmount: 1000, requiredWorkAmount: 200000 }
				]
			},
			research
		);

		expect(counts).toEqual({ done: 1, total: 3 });
	});

	it('reports zero done against the full catalogue when the lab is unreadable', () => {
		expect(countResearched(null, research)).toEqual({ done: 0, total: 3 });
	});
});
