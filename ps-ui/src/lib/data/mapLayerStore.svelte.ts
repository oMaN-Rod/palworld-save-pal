import {
	artifactsForLayers,
	getMapLayer,
	selectLayerEntries,
	type MapLayerArtifact,
	type MapLayerEntry,
	type MapLayerId,
	type MapLayerSelection,
	type RawArtifact
} from '$lib/components/map/layers/layerRegistry';
import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';

type MapLayerResponse = {
	layers?: Partial<Record<MapLayerArtifact, RawArtifact>>;
	error?: string;
};

const EMPTY: MapLayerSelection = { shape: 'keyed', points: [] };

class MapLayerStore {
	// $state.raw, not $state: an artifact holds thousands of markers read on every
	// map rebuild, and deep $state would proxy every one of those reads. Artifacts
	// are replaced wholesale, never mutated, so deep reactivity buys nothing here.
	#artifacts: Partial<Record<MapLayerArtifact, RawArtifact>> = $state.raw({});
	#views = new Map<MapLayerId, MapLayerSelection>();
	#pending = new Set<MapLayerArtifact>();
	// $state.raw and reactive so a row watching isLoading re-renders when it settles.
	#loading: ReadonlySet<MapLayerArtifact> = $state.raw(new Set());
	// The socket keys pending resolvers by message type alone, so two overlapping
	// get_map_layer requests would share one slot and the first would never
	// settle. Buffer into one batch and chain what does not fit, so at most one
	// is ever on the wire.
	#queue: Promise<void> = Promise.resolve();
	#epoch = 0;

	async getLayer<T extends MapLayerEntry = MapLayerEntry>(
		id: MapLayerId
	): Promise<MapLayerSelection<T>> {
		await this.#load([id]);
		return this.#view(id) as MapLayerSelection<T>;
	}

	async getLayers<K extends MapLayerId>(ids: readonly K[]): Promise<Record<K, MapLayerSelection>> {
		await this.#load(ids);
		const result = {} as Record<K, MapLayerSelection>;
		for (const id of ids) result[id] = this.#view(id);
		return result;
	}

	peek<T extends MapLayerEntry = MapLayerEntry>(id: MapLayerId): MapLayerSelection<T> | undefined {
		const { artifact } = getMapLayer(id);
		if (!this.#artifacts[artifact]) return undefined;
		return this.#view(id) as MapLayerSelection<T>;
	}

	isLoading(id: MapLayerId): boolean {
		return this.#loading.has(getMapLayer(id).artifact);
	}

	reset(): void {
		this.#epoch += 1;
		this.#artifacts = {};
		this.#views.clear();
		this.#pending.clear();
		this.#loading = new Set();
	}

	#view(id: MapLayerId): MapLayerSelection {
		const { artifact } = getMapLayer(id);
		const raw = this.#artifacts[artifact];
		if (!raw) return EMPTY;
		const cached = this.#views.get(id);
		if (cached) return cached;
		const view = selectLayerEntries(id, raw);
		this.#views.set(id, view);
		return view;
	}

	#load(ids: readonly MapLayerId[]): Promise<void> {
		const missing = artifactsForLayers(ids).filter((artifact) => !this.#artifacts[artifact]);
		if (missing.length === 0) return Promise.resolve();
		for (const artifact of missing) this.#pending.add(artifact);
		this.#loading = new Set([...this.#loading, ...missing]);
		const run = this.#queue.then(() => this.#flush());
		this.#queue = run.catch(() => {});
		return run;
	}

	async #flush(): Promise<void> {
		// The whole buffer clears its loading flag, including anything a batch ahead
		// of this one already cached -- otherwise that flag would never come down.
		const buffered = [...this.#pending];
		this.#pending.clear();
		const batch = buffered.filter((artifact) => !this.#artifacts[artifact]);
		const epoch = this.#epoch;
		try {
			if (batch.length === 0) return;

			let landed: Partial<Record<MapLayerArtifact, RawArtifact>> = {};
			try {
				const response = await sendAndWait<MapLayerResponse>(MessageType.GET_MAP_LAYER, {
					layers: batch
				});
				// A refusal answers under this same message type carrying `error`, so
				// sendAndWait resolves rather than throwing and it would pass unnoticed.
				if (response?.error) console.error('Error fetching map layers:', response.error);
				landed = response?.layers ?? {};
			} catch (error) {
				console.error('Error fetching map layers:', error);
			}
			if (epoch !== this.#epoch) return;

			// Cache an empty table for anything the response omitted, so a layer the
			// backend cannot serve is not re-asked on every map rebuild.
			const next = { ...this.#artifacts };
			for (const artifact of batch) next[artifact] = landed[artifact] ?? {};
			this.#artifacts = next;
		} finally {
			if (epoch === this.#epoch) {
				const stillLoading = new Set(this.#loading);
				for (const artifact of buffered) stillLoading.delete(artifact);
				this.#loading = stillLoading;
			}
		}
	}
}

export const mapLayers = new MapLayerStore();
