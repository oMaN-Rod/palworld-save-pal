// Tracks which cached meshes the map's layers are actively drawing, so the
// sweepers in the mesh libraries can free everything else ("dynamic
// offloading of out-of-sight assets") without ever disposing a geometry that
// a live InstancedMesh still references.
//
// Each layer replaces its scope's set wholesale after a rebuild, so entries
// left behind by a previous camera position / area naturally drop out. A
// swept mesh is not gone forever: the next rebuild that needs it re-requests
// it, and the browser HTTP cache makes the reload cheap.

const activeByScope = new Map<string, Set<string>>();

/** Replaces the active mesh-name set for one scope (e.g. 'scenery'). */
export function setActiveMeshes(scope: string, names: Iterable<string>): void {
	const next = new Set<string>();
	for (const name of names) next.add(name);
	activeByScope.set(scope, next);
}

/** Union of the named scopes' active sets; unknown scopes contribute nothing. */
export function activeMeshUnion(scopes: readonly string[]): Set<string> {
	const out = new Set<string>();
	for (const scope of scopes) {
		const set = activeByScope.get(scope);
		if (!set) continue;
		for (const name of set) out.add(name);
	}
	return out;
}

/** Drops a scope's registration (layer teardown). */
export function clearActiveMeshes(scope: string): void {
	activeByScope.delete(scope);
}
