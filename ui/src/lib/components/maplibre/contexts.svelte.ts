import { getContext, setContext } from 'svelte';
import type {
	Map as MaplibreMap,
	Marker as MaplibreMarker,
	SourceSpecification,
	AddLayerObject,
	IControl
} from 'maplibre-gl';
import type { Theme } from './types.js';

const MAP_CTX_KEY = Symbol('svlibre-map');
const SOURCE_CTX_KEY = Symbol('svlibre-source');
const LAYER_CTX_KEY = Symbol('svlibre-layer');
const MARKER_CTX_KEY = Symbol('svlibre-marker');

export class MapContext {
	map = $state<MaplibreMap | null>(null);
	loaded = $state(false);
	theme = $state<'light' | 'dark'>('light');

	private pendingOps: Array<() => void> = [];
	private userSources = new Set<string>();
	private userLayers: string[] = [];
	private userControls = new Set<IControl>();

	whenLoaded(fn: () => void | (() => void)): void {
		if (this.loaded && this.map) {
			fn();
		} else {
			this.pendingOps.push(fn);
		}
	}

	markLoaded(): void {
		this.loaded = true;
		const ops = this.pendingOps.splice(0);
		for (const op of ops) {
			op();
		}
	}

	// User sources/layers are removed by MapLibre on a style change, so this just
	// resets tracking state rather than removing them itself.
	markUnloaded(): void {
		this.loaded = false;
		this.userSources.clear();
		this.userLayers = [];
	}

	addSource(id: string, spec: SourceSpecification): void {
		if (!this.map) return;
		if (this.map.getSource(id)) return;
		this.map.addSource(id, spec);
		this.userSources.add(id);
	}

	removeSource(id: string): void {
		this.userSources.delete(id);
		if (!this.map) return;
		if (this.map.getSource(id)) {
			this.map.removeSource(id);
		}
	}

	// Tracking is for teardown only -- nothing re-adds these layers after a style change.
	addLayer(spec: AddLayerObject, beforeId?: string): void {
		if (!this.map) return;
		if (this.map.getLayer(spec.id)) return;
		this.map.addLayer(spec, beforeId);
		// A beforeId naming a layer that does not exist yet makes MapLibre fire an
		// ErrorEvent and return rather than throw, so tracking the id regardless
		// would record a layer that was never added.
		if (!this.map.getLayer(spec.id)) return;
		this.userLayers.push(spec.id);
	}

	removeLayer(id: string): void {
		this.userLayers = this.userLayers.filter((l) => l !== id);
		if (!this.map) return;
		if (this.map.getLayer(id)) {
			this.map.removeLayer(id);
		}
	}

	addControl(control: IControl, position?: string): void {
		if (!this.map) return;
		this.map.addControl(control, position as 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right');
		this.userControls.add(control);
	}

	removeControl(control: IControl): void {
		this.userControls.delete(control);
		if (!this.map) return;
		this.map.removeControl(control);
	}

	// For OS-level dark mode without a class-based system, pass theme='dark' explicitly.
	resolveTheme(theme: Theme): 'light' | 'dark' {
		if (theme === 'auto') {
			if (typeof window !== 'undefined') {
				return document.documentElement.classList.contains('dark') ? 'dark' : 'light';
			}
			return 'light';
		}
		return theme;
	}

	cleanup(): void {
		if (!this.map) return;

		for (let i = this.userLayers.length - 1; i >= 0; i--) {
			const id = this.userLayers[i];
			if (this.map.getLayer(id)) {
				this.map.removeLayer(id);
			}
		}
		this.userLayers = [];

		for (const id of this.userSources) {
			if (this.map.getSource(id)) {
				this.map.removeSource(id);
			}
		}
		this.userSources.clear();

		for (const control of this.userControls) {
			this.map.removeControl(control);
		}
		this.userControls.clear();

		this.map.remove();
		this.map = null;
		this.loaded = false;
	}
}

export class SourceContext {
	readonly id: string;

	constructor(id: string) {
		this.id = id;
	}
}

export class LayerContext {
	readonly id: string;
	readonly sourceId: string;

	constructor(id: string, sourceId: string) {
		this.id = id;
		this.sourceId = sourceId;
	}
}

export function setMapContext(ctx: MapContext): void {
	setContext(MAP_CTX_KEY, ctx);
}

export function getMapContext(): MapContext {
	const ctx = getContext<MapContext>(MAP_CTX_KEY);
	if (!ctx) {
		throw new Error('svlibre: <Map> context not found. Is this component inside a <Map>?');
	}
	return ctx;
}

export function setSourceContext(ctx: SourceContext): void {
	setContext(SOURCE_CTX_KEY, ctx);
}

export function getSourceContext(): SourceContext {
	const ctx = getContext<SourceContext>(SOURCE_CTX_KEY);
	if (!ctx) {
		throw new Error('svlibre: <Source> context not found. Is this component inside a <Source.*>?');
	}
	return ctx;
}

export function tryGetSourceContext(): SourceContext | undefined {
	return getContext<SourceContext | undefined>(SOURCE_CTX_KEY);
}

export function setLayerContext(ctx: LayerContext): void {
	setContext(LAYER_CTX_KEY, ctx);
}

export function getLayerContext(): LayerContext {
	const ctx = getContext<LayerContext>(LAYER_CTX_KEY);
	if (!ctx) {
		throw new Error('svlibre: <Layer> context not found. Is this component inside a <Layer.*>?');
	}
	return ctx;
}

export class MarkerContext {
	marker = $state<MaplibreMarker | null>(null);
	contentEl = $state<HTMLDivElement | null>(null);
}

export function setMarkerContext(ctx: MarkerContext): void {
	setContext(MARKER_CTX_KEY, ctx);
}

export function getMarkerContext(): MarkerContext {
	const ctx = getContext<MarkerContext>(MARKER_CTX_KEY);
	if (!ctx) {
		throw new Error('svlibre: <Marker> context not found. Is this component inside a <Marker>?');
	}
	return ctx;
}
