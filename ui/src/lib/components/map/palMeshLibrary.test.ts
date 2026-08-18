import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { palMeshFailed, palModelUrl, requestPalMesh } from './palMeshLibrary';

describe('palModelUrl', () => {
	it('resolves a manifest key to its hashed file under /models/pals', () => {
		const url = palModelUrl('anubis');
		expect(url).toMatch(/^\/models\/pals\/anubis_[0-9a-f]{6}\.glb$/);
	});

	// bossPalKey strips only the "boss_" prefix, so every real boss key reaches
	// this function mixed-case while manifest keys are lowercase. Without
	// normalisation this misses 100% of Pals, silently.
	it('resolves a mixed-case key, as bossPalKey actually produces', () => {
		expect(palModelUrl('Anubis')).toBe(palModelUrl('anubis'));
		expect(palModelUrl('Anubis')).not.toBeNull();
	});

	it('returns null for a key with no manifest entry', () => {
		expect(palModelUrl('__no_such_pal__')).toBeNull();
	});

	it('returns null for a human boss key that never existed', () => {
		expect(palModelUrl('definitely_not_a_pal')).toBeNull();
	});
});

describe('requestPalMesh', () => {
	// No "returns null while loading" case here: outside a browser FileLoader
	// throws on a relative URL and this library routes that to permanent failure,
	// so such a test would pass against an implementation that failed every key.
	it('marks a key with no manifest entry as permanently failed instead of fetching', () => {
		requestPalMesh('__no_such_pal__');
		expect(palMeshFailed('__no_such_pal__')).toBe(true);
	});

	// Keys arrive mixed-case while the manifest is lowercase, so the caches must
	// key on the same normalised form palModelUrl looks up with -- otherwise one
	// key occupies two identities and a recorded failure is invisible. The probe
	// key is unique to this case: reusing an already-failed one would pass against
	// the un-normalised implementation too.
	it('records a failure under the normalised key, whatever casing the caller used', () => {
		requestPalMesh('CasingProbe_NotAPal');
		expect(palMeshFailed('casingprobe_notapal')).toBe(true);
		expect(palMeshFailed('CasingProbe_NotAPal')).toBe(true);
	});
});

// Real-artifact test; skips when the bake has not been run.
const REAL = resolve(__dirname, '../../../../static/models/pals');
const MANIFEST_PATH = resolve(__dirname, '../../../../../data/json/pal_meshes.json');
describe.skipIf(!existsSync(REAL))('exported Pal glb (real artifact)', () => {
	function firstGlb(): Buffer {
		const manifest = JSON.parse(readFileSync(MANIFEST_PATH, 'utf8'));
		const file = manifest[Object.keys(manifest)[0]].file;
		return readFileSync(resolve(REAL, file));
	}

	function json(glb: Buffer): any {
		const len = glb.readUInt32LE(12);
		return JSON.parse(glb.subarray(20, 20 + len).toString('utf8'));
	}

	it('carries exactly POSITION, NORMAL and TEXCOORD_0 -- no UV1-7, no skinning, no tangents', () => {
		const j = json(firstGlb());
		const attrs = new Set<string>();
		for (const mesh of j.meshes) {
			for (const prim of mesh.primitives) {
				for (const a of Object.keys(prim.attributes)) attrs.add(a);
			}
		}
		expect([...attrs].sort()).toEqual(['NORMAL', 'POSITION', 'TEXCOORD_0']);
	});

	it('embeds its textures rather than referencing external files', () => {
		const j = json(firstGlb());
		expect(j.images.length).toBeGreaterThan(0);
		for (const img of j.images) {
			expect(img.uri).toBeUndefined();
			expect(img.bufferView).toBeTypeOf('number');
		}
	});

	it('is meshopt-compressed, which is why the loader must register MeshoptDecoder', () => {
		const j = json(firstGlb());
		expect(j.extensionsRequired).toContain('EXT_meshopt_compression');
	});

	// gltfpack drops EXT_texture_webp from the extension lists while keeping
	// mimeType image/webp, so assert on the bytes rather than the declaration.
	it('embeds lossy webp image bytes', () => {
		const glb = firstGlb();
		const j = json(glb);
		const binStart = 20 + glb.readUInt32LE(12) + 8;
		for (const img of j.images) {
			const view = j.bufferViews[img.bufferView];
			const start = binStart + (view.byteOffset ?? 0);
			const bytes = glb.subarray(start, start + view.byteLength);
			expect(bytes.subarray(0, 4).toString('latin1')).toBe('RIFF');
			expect(bytes.subarray(8, 12).toString('latin1')).toBe('WEBP');
			expect(bytes.includes(Buffer.from('VP8L', 'latin1'))).toBe(false);
		}
	});
});
