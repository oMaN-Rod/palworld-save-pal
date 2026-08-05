import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type BaseStructure, type Footprint } from '$types';

type BaseStructuresResponse = {
	base_id?: string;
	structures?: BaseStructure[];
	error?: string;
};

class BaseStructures {
	private loadingFootprints = false;
	private inflight = new Set<string>();
	private epoch = 0;
	// sendAndWait keys pending resolvers by message type, so two overlapping
	// GET_BASE_STRUCTURES requests would share one slot and the first would never
	// settle. Chain requests so at most one is ever on the wire.
	private queue: Promise<void> = Promise.resolve();
	private byBase: Record<string, BaseStructure[]> = $state({});

	footprints: Record<string, Footprint> = $state({});

	async loadFootprints(): Promise<void> {
		if (this.loadingFootprints || Object.keys(this.footprints).length > 0) return;
		this.loadingFootprints = true;
		try {
			this.footprints = await sendAndWait<Record<string, Footprint>>(
				MessageType.GET_MAP_OBJECT_FOOTPRINTS
			);
		} catch (error) {
			console.error('Error fetching map object footprints:', error);
		} finally {
			this.loadingFootprints = false;
		}
	}

	async load(baseId: string): Promise<void> {
		if (this.byBase[baseId] || this.inflight.has(baseId)) return;
		this.inflight.add(baseId);
		const epoch = this.epoch;
		const run = this.queue.then(async () => {
			try {
				const response = await sendAndWait<BaseStructuresResponse>(
					MessageType.GET_BASE_STRUCTURES,
					{ base_id: baseId }
				);
				if (epoch !== this.epoch) return;
				if (response?.base_id && response.base_id !== baseId) return;
				// Errors ("No save file loaded", malformed request) come back under the same
				// message type and carry no structure list; cache empty so a moving map does
				// not re-ask on every frame.
				this.byBase[baseId] = Array.isArray(response?.structures) ? response.structures : [];
			} catch (error) {
				console.error('Error fetching base structures:', error);
				if (epoch === this.epoch) this.byBase[baseId] = [];
			} finally {
				this.inflight.delete(baseId);
			}
		});
		this.queue = run.catch(() => {});
		return run;
	}

	for(baseId: string): BaseStructure[] {
		return this.byBase[baseId] ?? [];
	}

	reset(): void {
		this.epoch += 1;
		this.byBase = {};
		this.inflight.clear();
	}
}

export const baseStructuresData = new BaseStructures();
