import { blueprintsData } from '$lib/data/blueprints.svelte';
import type {
	BlueprintFinding,
	BlueprintHeader,
	BlueprintStructureGeometry,
	PlacementAnchor,
	PlaceBlueprintResponse
} from '$types';

class Placement {
	active = $state(false);
	handle = $state<string | null>(null);
	header = $state<BlueprintHeader | null>(null);
	geometry = $state<BlueprintStructureGeometry[]>([]);
	anchor = $state<PlacementAnchor>({ x: 0, y: 0, z: 0, yaw: 0 });
	targetGuild = $state('');
	targetPlayer = $state('');
	overrideWarnings = $state(false);
	findings = $state<BlueprintFinding[]>([]);
	hasBlocking = $state(false);

	enter(handle: string, header: BlueprintHeader): void {
		this.active = true;
		this.handle = handle;
		this.header = header;
		this.geometry = [];
		this.anchor = { x: 0, y: 0, z: 0, yaw: 0 };
		this.targetGuild = '';
		this.targetPlayer = '';
		this.overrideWarnings = false;
		this.findings = [];
		this.hasBlocking = false;
	}

	setAnchor(anchor: PlacementAnchor): void {
		this.anchor = anchor;
	}

	async runValidate(): Promise<void> {
		if (!this.handle || !this.targetGuild) return;
		const res = await blueprintsData.validate(this.handle, this.anchor, this.targetGuild);
		this.findings = res.findings;
		this.hasBlocking = res.has_blocking;
	}

	async commit(): Promise<PlaceBlueprintResponse> {
		if (!this.handle) throw new Error('no blueprint to place');
		return blueprintsData.place(
			this.handle,
			this.anchor,
			this.targetGuild,
			this.targetPlayer,
			this.overrideWarnings
		);
	}

	exit(): void {
		this.active = false;
		this.handle = null;
		this.header = null;
		this.geometry = [];
		this.findings = [];
		this.hasBlocking = false;
	}
}

export const placementState = new Placement();
