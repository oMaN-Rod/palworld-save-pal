import {
	artifactsForLayers,
	getMapLayer,
	selectLayerEntries,
	type MapLayerArtifact,
	type MapLayerEntry,
	type MapLayerId,
	type MapLayerSelection,
	type RawArtifact
} from '$lib/components/map/layerRegistry';
import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';

type MapLayerResponse = {
	layers?: Partial<Record<MapLayerArtifact, RawArtifact>>;
	error?: string;
};

const EMPTY: MapLayerSelection = { shape: 'keyed', points: [] };

class MapLayerStore {
	// $state.raw, not $state: an artifact holds thousands of markers whose fields
	// are read on every map rebuild, and under a deep $state every one of those
	// reads goes through Svelte's proxy -- the same shape that cost ~127 s per
	// load on bulk save data. Artifacts are replaced wholesale, never mutated, so
	// deep reactivity buys nothing here.
	#artifacts: Partial<Record<MapLayerArtifact, RawArtifact>> = $state.raw({});
	// Selections are derived from an immutable artifact, so once computed they
	// stay valid until reset().
	#views = new Map<MapLayerId, MapLayerSelection>();
	// The next batch to send. Drained the moment it goes on the wire.
	#pending = new Set<MapLayerArtifact>();
	// Requested and not yet settled, whether or not it has reached the wire.
	// Reactive so a row watching isLoading re-renders when the answer lands.
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

	/** Several layers in one request, keyed back by layer id. */
	async getLayers<K extends MapLayerId>(ids: readonly K[]): Promise<Record<K, MapLayerSelection>> {
		await this.#load(ids);
		const result = {} as Record<K, MapLayerSelection>;
		for (const id of ids) result[id] = this.#view(id);
		return result;
	}

	/** Entries already cached for `id`, without triggering a fetch. */
	peek<T extends MapLayerEntry = MapLayerEntry>(id: MapLayerId): MapLayerSelection<T> | undefined {
		const { artifact } = getMapLayer(id);
		if (!this.#artifacts[artifact]) return undefined;
		return this.#view(id) as MapLayerSelection<T>;
	}

	/** True from the moment a layer is asked for until its artifact lands or the
	 *  request fails, so a row can tell "still coming" from "never asked". */
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
		// Everything buffered while the previous request was on the wire goes out
		// together; callers that joined this batch find it already drained. The
		// whole buffer clears its loading flag, including anything a batch ahead
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
