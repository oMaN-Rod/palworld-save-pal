import { type MapArea } from '../../geo/utils';
import { PORTAL_RADIUS_CM } from './palPortal';
import { buildRingFC } from '../mesh/ringFC';
import type { PalBoss, PalPredator } from './palLayer';
import type { PalRingKind } from '../objects/mapObjectPortal';

type PalPortalRingSource = { x: number; y: number; state: PalRingKind; defeated: boolean };

// Alpha and boss share CORE_COLOR (see palRingColor), so both are tagged
// 'boss' for the ring's color lookup -- a separate 'alpha' arm would resolve
// to the same color and add nothing.
export function buildPalPortalFC(
	bosses: PalBoss[],
	predators: PalPredator[],
	area: MapArea,
	palScale: number
): GeoJSON.FeatureCollection {
	const items: PalPortalRingSource[] = [
		...bosses.map((b) => ({ x: b.x, y: b.y, state: 'boss' as const, defeated: b.defeated })),
		...predators.map((p) => ({ x: p.x, y: p.y, state: 'predator' as const, defeated: false }))
	];
	return buildRingFC(items, area, PORTAL_RADIUS_CM * palScale, (item) => ({
		state: item.state,
		defeated: item.defeated
	}));
}
