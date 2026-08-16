// Runs against the shipped data/json artifacts rather than fixtures. The
// array-vs-object split, the missing instance_id in camps and the removal of
// chests.json are all properties of the real files, and a fixture would just
// restate whatever the code already assumes.
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { MAP_LAYERS, selectLayerEntries, type MapLayerId, type RawArtifact } from './layerRegistry';
import { buildMapLayerFC } from './mapLayerFeatures';

const __dirname = dirname(fileURLToPath(import.meta.url));
const JSON_DIR = resolve(__dirname, '../../../../../data/json');

const artifactPath = (name: string) => resolve(JSON_DIR, `${name}.json`);

function loadArtifact(name: string): RawArtifact {
	return JSON.parse(readFileSync(artifactPath(name), 'utf8'));
}

const artifactOf = (id: MapLayerId) => MAP_LAYERS.find((layer) => layer.id === id)!.artifact;

describe('every registered layer is backed by a file that exists', () => {
	for (const layer of MAP_LAYERS) {
		it(`${layer.id} -> ${layer.artifact}.json`, () => {
			expect(existsSync(artifactPath(layer.artifact))).toBe(true);
		});
	}
});

// chests.json was deleted for weight; the layer had to go with it, or the panel
// would offer a toggle that can never draw anything.
describe('chests', () => {
	it('has no file and no layer', () => {
		expect(existsSync(artifactPath('chests'))).toBe(false);
		expect(MAP_LAYERS.map((layer) => layer.artifact)).not.toContain('chests');
	});
});

const SHAPES: Array<[MapLayerId, 'keyed' | 'list']> = [
	['tower_boss', 'keyed'],
	['journals', 'keyed'],
	['dungeons', 'keyed'],
	['camps', 'list'],
	['eggs', 'list']
];

describe.each(SHAPES)('%s artifact', (id, expectedShape) => {
	const raw = loadArtifact(artifactOf(id));

	it(`is really ${expectedShape} on disk`, () => {
		expect(Array.isArray(raw)).toBe(expectedShape === 'list');
	});

	it('is read back with the shape the selector reports', () => {
		expect(selectLayerEntries(id, raw).shape).toBe(expectedShape);
	});

	it('yields every entry', () => {
		const { points } = selectLayerEntries(id, raw);
		const total = Array.isArray(raw) ? raw.length : Object.keys(raw).length;
		expect(points).toHaveLength(total);
	});

	it('gives every entry a distinct key', () => {
		const { points } = selectLayerEntries(id, raw);
		expect(new Set(points.map((p) => p.key)).size).toBe(points.length);
	});

	it('places every entry with finite coordinates across the two map areas', () => {
		const selection = selectLayerEntries(id, raw);
		const placed = ['MainMap', 'Tree'].flatMap(
			(area) => buildMapLayerFC(id, selection, area as 'MainMap' | 'Tree').features
		);
		const positioned = selection.points.filter(
			(p) => Number.isFinite(p.entry.x) && Number.isFinite(p.entry.y)
		);
		expect(placed).toHaveLength(positioned.length);
		for (const feature of placed) {
			expect(feature.geometry.coordinates.every(Number.isFinite)).toBe(true);
			expect(feature.properties.name).not.toContain('undefined');
			expect(feature.properties.name).not.toBe('');
		}
	});
});

// 8 of 59 camp entries ship without instance_id, so identity has to fall through
// to another field rather than collapsing onto one shared key.
describe('camps identity fallback', () => {
	const raw = loadArtifact('camps') as Array<Record<string, unknown>>;

	it('has entries with no instance_id', () => {
		expect(raw.some((entry) => entry.instance_id === undefined)).toBe(true);
	});

	it('still keys every one of them distinctly', () => {
		const { points } = selectLayerEntries('camps', raw);
		expect(new Set(points.map((p) => p.key)).size).toBe(raw.length);
	});
});

// get_map_layer folds localized_name in from l10n/<lang>/<artifact>.json at
// request time, so the raw artifact on disk carries none. towers is the only
// new layer with such a table; notes and camps have none and must fall back.
describe('localized_name coverage', () => {
	it('comes from the l10n table, which covers every towers entry', () => {
		const raw = loadArtifact('towers') as Record<string, Record<string, unknown>>;
		expect(Object.values(raw).every((entry) => entry.localized_name === undefined)).toBe(true);

		const l10n = JSON.parse(
			readFileSync(resolve(JSON_DIR, 'l10n/en/towers.json'), 'utf8')
		) as Record<string, { localized_name?: string }>;
		for (const key of Object.keys(raw)) {
			expect(l10n[key]?.localized_name).toBeTruthy();
		}
	});

	it('has no table for the array artifacts, which the merge skips', () => {
		expect(existsSync(resolve(JSON_DIR, 'l10n/en/camps.json'))).toBe(false);
		expect(existsSync(resolve(JSON_DIR, 'l10n/en/eggs_spawners.json'))).toBe(false);
		expect(existsSync(resolve(JSON_DIR, 'l10n/en/notes.json'))).toBe(false);
	});

	it('is absent from notes and camps, which fall back rather than render undefined', () => {
		for (const id of ['journals', 'camps'] as const) {
			const selection = selectLayerEntries(id, loadArtifact(artifactOf(id)));
			const fc = buildMapLayerFC(id, selection, 'MainMap');
			for (const feature of fc.features) {
				expect(feature.properties.name).toBeTruthy();
			}
		}
	});
});
