import { ASSET_DATA_PATH } from '$lib/constants';
import type { RelicRankData } from '$lib/data/relic.svelte';

/** The 13 effigy relic types in the game's own `EPalRelicType` order -- the
 *  order the Statue of Power lists them and the icon numbering follows. */
export const RELIC_ORDER = [
	'capture_power',
	'hunger_reduction',
	'swim_speed',
	'food_decay_reduction',
	'jump_power',
	'glider_speed',
	'climb_speed',
	'status_ailment_resist',
	'stamina_reduction',
	'sphere_homing',
	'exp_bonus',
	'rainbow_passive_rate',
	'move_speed'
] as const;

export type RelicKey = (typeof RELIC_ORDER)[number];

/** `capture_power` maps to the `capture_rate` stat; every other key maps 1:1. */
export function statKeyFor(relicKey: string): string {
	return relicKey === 'capture_power' ? 'capture_rate' : relicKey;
}

/** The pal-face item icon for a relic type, numbered by `RELIC_ORDER` position:
 *  00 Lifmunk (no suffix), 01 Lamball, 02 Pengullet, 03 Munchill, 04 Rooby,
 *  05 Herbil, 06 Tanzee, 07 Depresso, 08 Cattiva, 09 Lunaris, 10 Relaxaurus,
 *  11 Yakumo, 12 Mimog. Unknown keys fall back to the Lifmunk icon. */
export function relicIconPath(relicKey: string): string {
	const index = RELIC_ORDER.indexOf(relicKey as RelicKey);
	const suffix = index > 0 ? `_${String(index).padStart(2, '0')}` : '';
	return `${ASSET_DATA_PATH}/img/t_itemicon_relic${suffix}.webp`;
}

/** Zero-padded display index (`#00`..`#12`), `--` for an unknown type. */
export function relicIndexLabel(relicKey: string): string {
	const index = RELIC_ORDER.indexOf(relicKey as RelicKey);
	return index >= 0 ? String(index).padStart(2, '0') : '--';
}

/** Rank earned for `count` invested effigies, by walking the cumulative
 *  `per_rank` thresholds. Mirrors `psp-core::domain::relic::rank_for_count`
 *  exactly -- the Rust side is the authority; keep the walks identical. */
export function rankForCount(perRank: number[], count: number): number {
	let rank = 0;
	let cumulative = 0;
	for (const step of perRank) {
		cumulative += step;
		if (count >= cumulative) {
			rank += 1;
		} else {
			break;
		}
	}
	return rank;
}

/** Held-count clamp the backend enforces (`apply_relic_possess_counts`):
 *  the UI mirrors it so a staged value never silently changes on save. */
export function clampCount(count: number, cumulativeMax: number): number {
	return Math.max(0, Math.min(count, cumulativeMax));
}

/** The `status_point_list` patch that syncs every relic stat's rank to its
 *  staged count, the way PalSavTools' ability PUT does: setting a type's
 *  count invests that many effigies, so the rank follows the same thresholds.
 *  Types without loaded relic data are skipped -- their caps are unknown. */
export function deriveStatusPatches(
	values: Record<string, number>,
	relics: Record<string, RelicRankData>
): Record<string, number> {
	const patches: Record<string, number> = {};
	for (const relicKey of RELIC_ORDER) {
		const entry = relics[relicKey];
		if (!entry) continue;
		patches[statKeyFor(relicKey)] = rankForCount(entry.per_rank, values[relicKey] ?? 0);
	}
	return patches;
}

/** True when the count loaded from the save exceeds the type's cap -- data no
 *  legit gameplay can produce (another tool wrote it). The backend clamps such
 *  a value on save; the UI badges it first so the edit is an informed one. */
export function isIllegalCount(count: number, cumulativeMax: number | undefined): boolean {
	return cumulativeMax !== undefined && count > cumulativeMax;
}
