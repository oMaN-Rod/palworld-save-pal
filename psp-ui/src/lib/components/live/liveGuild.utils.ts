import type {
	GameGuildLabJson,
	GameInventoryContainerJson,
	GamePalJson
} from '$states/gameState.svelte';
import type { Technology as LabResearch } from '$types';

export type LiveGuildView = 'members' | 'lab' | 'pals' | 'storage' | 'chest';

export interface ResearchGuildShape {
	lab_research_data: { research_id: string; work_amount: number }[];
}

export function toResearchGuildShape(lab: GameGuildLabJson | null): ResearchGuildShape {
	return {
		lab_research_data: (lab?.research ?? [])
			.filter((row) => !!row.researchId)
			.map((row) => ({ research_id: row.researchId as string, work_amount: row.workAmount ?? 0 }))
	};
}

export function partitionContainers(containers: GameInventoryContainerJson[]): {
	chest: GameInventoryContainerJson[];
	storage: GameInventoryContainerJson[];
} {
	const isChest = (container: GameInventoryContainerJson) => container.type === 'GuildChest';
	return {
		chest: containers.filter(isChest),
		storage: containers.filter((container) => !isChest(container))
	};
}

export interface PalSeat {
	position: number;
	pal: GamePalJson | undefined;
}

export function seatPals(pals: GamePalJson[], slotCount: number, slotBase = 0): PalSeat[] {
	const byPosition = new Map<number, GamePalJson>();
	for (const pal of pals) {
		const position = pal.slotIndex - slotBase;
		if (position >= 0 && !byPosition.has(position)) byPosition.set(position, pal);
	}
	const size = Math.max(slotCount, ...[...byPosition.keys()].map((position) => position + 1), 0);
	return Array.from({ length: size }, (_, position) => ({ position, pal: byPosition.get(position) }));
}

const TICKS_PER_MS = 10_000;
// Milliseconds between 0001-01-01, which FDateTime counts from, and the Unix epoch.
const EPOCH_OFFSET_MS = 62_135_596_800_000;

export function lastOnlineDate(ticks: number | null): Date | null {
	if (ticks === null || !Number.isFinite(ticks) || ticks <= 0) return null;
	const date = new Date(ticks / TICKS_PER_MS - EPOCH_OFFSET_MS);
	const year = date.getUTCFullYear();
	return year >= 2000 && year <= 2100 ? date : null;
}

export function countResearched(
	lab: GameGuildLabJson | null,
	research: Record<string, LabResearch>
): { done: number; total: number } {
	const required = new Map(
		Object.values(research).map((entry) => [entry.id, entry.details.work_amount ?? 0])
	);
	let done = 0;
	for (const row of lab?.research ?? []) {
		if (!row.researchId) continue;
		const need = required.get(row.researchId);
		if (need !== undefined && need > 0 && (row.workAmount ?? 0) >= need) done += 1;
	}
	return { done, total: required.size };
}
