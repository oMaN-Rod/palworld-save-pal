import type { LibraryMod, ModProfile, ProfileMod, ReorderKind } from '$types';
import { displayName } from './modList';

export interface OrderRow {
	entry: ProfileMod;
	mod: LibraryMod;
}

const kindTypes: Record<ReorderKind, readonly string[]> = {
	ue4ss: ['ue4ss', 'hybrid'],
	palschema: ['palschema']
};

/** Ties break on the raw mod id, as the server's `ORDER BY load_order, mod_id` does. */
function byLoadOrder(a: ProfileMod, b: ProfileMod): number {
	if (a.load_order !== b.load_order) return a.load_order - b.load_order;
	return a.mod_id < b.mod_id ? -1 : a.mod_id > b.mod_id ? 1 : 0;
}

function rowsOfTypes(
	profile: ModProfile,
	mods: LibraryMod[],
	types: readonly string[]
): OrderRow[] {
	const byId = new Map(mods.map((mod) => [mod.id, mod]));
	return [...profile.mods].sort(byLoadOrder).flatMap((entry) => {
		const mod = byId.get(entry.mod_id);
		return mod && types.includes(mod.mod_type) ? [{ entry, mod }] : [];
	});
}

export function orderRows(profile: ModProfile, mods: LibraryMod[], kind: ReorderKind): OrderRow[] {
	return rowsOfTypes(profile, mods, kindTypes[kind]);
}

export function alphabeticalRows(
	profile: ModProfile,
	mods: LibraryMod[],
	modTypes: string[]
): OrderRow[] {
	return rowsOfTypes(profile, mods, modTypes).sort((a, b) =>
		displayName(a.mod).localeCompare(displayName(b.mod), undefined, { sensitivity: 'base' })
	);
}

export function moveId(ids: string[], index: number, delta: -1 | 1): string[] | null {
	const to = index + delta;
	if (index < 0 || index >= ids.length || to < 0 || to >= ids.length) return null;
	const next = [...ids];
	[next[index], next[to]] = [next[to], next[index]];
	return next;
}

/** The server refuses an order that omits any entry of the kind, so an unloaded mod blocks reordering. */
export function libraryCovers(profile: ModProfile, mods: LibraryMod[]): boolean {
	const known = new Set(mods.map((mod) => mod.id));
	return profile.mods.every((entry) => known.has(entry.mod_id));
}
