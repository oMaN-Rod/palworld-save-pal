import type maplibregl from 'maplibre-gl';

let counter = 0;

export function generateId(prefix: string = 'svlibre'): string {
	return `${prefix}-${++counter}`;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function addEventHandler(
	evented: maplibregl.Evented,
	type: string,
	listener: (...args: any[]) => void
): () => void {
	evented.on(type, listener);
	return () => {
		evented.off(type, listener);
	};
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function addLayerEventHandler(
	map: maplibregl.Map,
	type: string,
	layerId: string,
	listener: (...args: any[]) => void
): () => void {
	map.on(type as keyof maplibregl.MapLayerEventType, layerId, listener as () => void);
	return () => {
		map.off(type as keyof maplibregl.MapLayerEventType, layerId, listener as () => void);
	};
}
