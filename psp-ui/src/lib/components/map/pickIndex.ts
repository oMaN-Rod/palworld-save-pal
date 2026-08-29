// Instances are bucketed by mesh and colour, so an instance's position within
// its own InstancedMesh is not unique. The pick pass needs one flat namespace
// across every bucket; this owns that mapping.
export class PickIndex {
	private keys: string[] = [];

	reset(): void {
		this.keys.length = 0;
	}

	add(ids: string[]): number {
		const base = this.keys.length;
		for (const id of ids) this.keys.push(id);
		return base;
	}

	keyAt(index: number): string | null {
		return this.keys[index] ?? null;
	}

	get size(): number {
		return this.keys.length;
	}
}
