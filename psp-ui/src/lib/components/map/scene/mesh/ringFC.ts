import { pixelCirclePolygon } from '../../geo/mercator';
import { cmPerPx, worldToPixel, type MapArea } from '../../geo/utils';

export const RING_SEGMENTS = 48;

// `radiusCm` takes a function when one collection mixes item kinds that scale
// independently, as fast travel statues and watchtowers do.
export function buildRingFC<T extends { x: number; y: number }>(
	items: T[],
	area: MapArea,
	radiusCm: number | ((item: T) => number),
	props: (item: T) => GeoJSON.GeoJsonProperties
): GeoJSON.FeatureCollection {
	const cm = cmPerPx(area);
	const radiusOf = typeof radiusCm === 'function' ? radiusCm : () => radiusCm;
	const features: GeoJSON.Feature<GeoJSON.Polygon>[] = items.map((item, i) => {
		const [cx, cy] = worldToPixel(item.x, item.y, area);
		return {
			type: 'Feature',
			id: i,
			geometry: {
				type: 'Polygon',
				coordinates: [pixelCirclePolygon(cx, cy, radiusOf(item) / cm, RING_SEGMENTS)]
			},
			properties: props(item)
		};
	});
	return { type: 'FeatureCollection', features };
}
