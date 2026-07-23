import { DEFAULT_STRUCTURE_FOOTPRINT } from './features';
import type { BaseStructure, Building, Footprint, PlayerSummary } from '$types';

export type StructureInfo = {
	name: string;
	typeA: string;
	description?: string;
	icon?: string;
	hp: number;
	hpMax: number;
	sizeM: { x: number; y: number; z: number };
	builder?: string;
	rank?: number;
	materials?: { id: string; count: number }[];
};

export function structureInfo(
	s: BaseStructure,
	footprints: Record<string, Footprint>,
	buildings: Record<string, Building>,
	summaries: Record<string, PlayerSummary>
): StructureInfo {
	const fp = footprints[s.map_object_id] ?? DEFAULT_STRUCTURE_FOOTPRINT;
	const key = s.map_object_id.toLowerCase();
	const building = Object.entries(buildings).find(([k]) => k.toLowerCase() === key)?.[1];
	const uid = s.build_player_uid?.toLowerCase();
	const summary = uid
		? Object.entries(summaries).find(([k]) => k.toLowerCase() === uid)?.[1]
		: undefined;

	return {
		name: building?.localized_name || s.map_object_id,
		typeA: fp.typeA,
		description: building?.description,
		icon: building?.icon,
		hp: s.hp_current,
		hpMax: s.hp_max,
		sizeM: {
			x: (fp.sx * s.scale_x) / 100,
			y: (fp.sy * s.scale_y) / 100,
			z: (fp.sz * s.scale_z) / 100
		},
		builder: summary?.nickname,
		rank: building?.rank,
		materials: building?.materials
	};
}
