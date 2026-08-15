import type { Boss, PredatorSpawn, Spawn } from '$types';

export type BossRow = Boss & { rowKey: string };
export type PredatorRow = PredatorSpawn & { rowKey: string };

export type SpawnPartition = {
	alpha: BossRow[];
	boss: BossRow[];
	predator: PredatorRow[];
};

// `unknown`, not `never`: TS cannot narrow a Boss to `never` just because both
// of its literal spawn_type values were rejected above. The branch is real at
// runtime anyway -- the wire payload is untyped JSON.
function warnUnknownSpawnType(spawn: unknown, rowKey: string): void {
	console.warn(`Dropping spawn "${rowKey}" with unrecognized spawn_type`, spawn);
}

/** Splits bosses.json's merged spawn table by `spawn_type`, the sole source
 *  of truth for which marker a spawn renders as. An unrecognized spawn_type
 *  is dropped with a warning rather than silently defaulting into `boss`. */
export function partitionSpawns(spawns: Record<string, Spawn>): SpawnPartition {
	const alpha: BossRow[] = [];
	const boss: BossRow[] = [];
	const predator: PredatorRow[] = [];
	for (const [rowKey, spawn] of Object.entries(spawns)) {
		if (spawn.spawn_type === 'predator') predator.push({ ...spawn, rowKey });
		else if (spawn.spawn_type === 'alpha') alpha.push({ ...spawn, rowKey });
		else if (spawn.spawn_type === 'boss') boss.push({ ...spawn, rowKey });
		else warnUnknownSpawnType(spawn, rowKey);
	}
	return { alpha, boss, predator };
}
