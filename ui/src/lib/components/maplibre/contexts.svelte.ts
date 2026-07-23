import { getContext, setContext } from 'svelte';
import type {
	Map as MaplibreMap,
	Marker as MaplibreMarker,
	SourceSpecification,
	AddLayerObject,
	IControl
} from 'maplibre-gl';
import type { Theme } from './types.js';

// --- Context Keys ---

const MAP_CTX_KEY = Symbol('svlibre-map');
const SOURCE_CTX_KEY = Symbol('svlibre-source');
const LAYER_CTX_KEY = Symbol('svlibre-layer');
const MARKER_CTX_KEY = Symbol('svlibre-marker');

// --- MapContext ---

export class MapContext {
	map = $state<MaplibreMap | null>(null);
	loaded = $state(false);
	theme = $state<'light' | 'dark'>('light');

	private pendingOps: Array<() => void> = [];
	private userSources = new Set<string>();
	private userLayers: string[] = [];
	private userControls = new Set<IControl>();

	/**
	 * Queue an operation until the style is loaded, or execute immediately if ready.
	 * Returns a cleanup function if the callback returns one.
	 */
	whenLoaded(fn: () => void | (() => void)): void {
		if (this.loaded && this.map) {
			fn();
		} else {
			this.pendingOps.push(fn);
		}
	}

	/**
	 * Called when the map style finishes loading.
	 * Flushes all pending operations.
	 */
	markLoaded(): void {
		this.loaded = true;
		const ops = this.pendingOps.splice(0);
		for (const op of ops) {
			op();
		}
	}

	/**
	 * Called when style changes. User sources/layers are removed by MapLibre,
	 * so we just reset tracking state and mark as not loaded.
	 */
	markUnloaded(): void {
		this.loaded = false;
		this.userSources.clear();
		this.userLayers = [];
	}

	/**
	 * Add a source to the map and track it for cleanup.
	 */
	addSource(id: string, spec: SourceSpecification): void {
		if (!this.map) return;
		if (this.map.getSource(id)) return;
		this.map.addSource(id, spec);
		this.userSources.add(id);
	}

	/**
	 * Remove a tracked source from the map.
	 */
	removeSource(id: string): void {
		this.userSources.delete(id);
		if (!this.map) return;
		if (this.map.getSource(id)) {
			this.map.removeSource(id);
		}
	}

	/**
	 * Add a layer to the map and track it for cleanup.
	 * Layers are tracked in order for correct re-addition after style changes.
	 */
	addLayer(spec: AddLayerObject, beforeId?: string): void {
		if (!this.map) return;
		if (this.map.getLayer(spec.id)) return;
		this.map.addLayer(spec, beforeId);
		this.userLayers.push(spec.id);
	}

	/**
	 * Remove a tracked layer from the map.
	 */
	removeLayer(id: string): void {
		this.userLayers = this.userLayers.filter((l) => l !== id);
		if (!this.map) return;
		if (this.map.getLayer(id)) {
			this.map.removeLayer(id);
		}
	}

	/**
	 * Track a control added to the map for cleanup.
	 */
	addControl(control: IControl, position?: string): void {
		if (!this.map) return;
		this.map.addControl(control, position as 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right');
		this.userControls.add(control);
	}

	/**
	 * Remove a tracked control from the map.
	 */
	removeControl(control: IControl): void {
		this.userControls.delete(control);
		if (!this.map) return;
		this.map.removeControl(control);
	}

	/**
	 * Resolve the effective theme.
	 * Uses .dark class on <html> (Tailwind / mode-watcher convention).
	 * For OS-level dark mode without a class-based system, pass theme='dark' explicitly.
	 */
	resolveTheme(theme: Theme): 'light' | 'dark' {
		if (theme === 'auto') {
			if (typeof window !== 'undefined') {
				return document.documentElement.classList.contains('dark') ? 'dark' : 'light';
			}
			return 'light';
		}
		return theme;
	}

	/**
	 * Clean up all tracked resources. Called when the Map component is destroyed.
	 */
	cleanup(): void {
		if (!this.map) return;

		// Remove layers in reverse order (dependencies)
		for (let i = this.userLayers.length - 1; i >= 0; i--) {
			const id = this.userLayers[i];
			if (this.map.getLayer(id)) {
				this.map.removeLayer(id);
			}
		}
		this.userLayers = [];

		// Remove sources
		for (const id of this.userSources) {
			if (this.map.getSource(id)) {
				this.map.removeSource(id);
			}
		}
		this.userSources.clear();

		// Remove controls
		for (const control of this.userControls) {
			this.map.removeControl(control);
		}
		this.userControls.clear();

		this.map.remove();
		this.map = null;
		this.loaded = false;
	}
}

// --- SourceContext ---

export class SourceContext {
	readonly id: string;

	constructor(id: string) {
		this.id = id;
	}
}

// --- LayerContext ---

export class LayerContext {
	readonly id: string;
	readonly sourceId: string;

	constructor(id: string, sourceId: string) {
		this.id = id;
		this.sourceId = sourceId;
	}
}

// --- Context Accessors ---

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

// --- MarkerContext ---

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
