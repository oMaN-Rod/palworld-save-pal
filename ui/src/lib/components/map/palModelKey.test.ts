import { describe, expect, it } from 'vitest';
import { resolvePalModelKey } from './palModelKey';

// An explicit inventory rather than the shipped manifest: these cases are about
// the resolution rule, and pinning them to the last bake would make them fail for
// reasons unrelated to it.
const INVENTORY = new Set([
	'anubis',
	'blackpuppy',
	'cubeturtle',
	'cubeturtle_neutral',
	'elecpanda',
	'grimgirl',
	'kingwhale',
	'nightlady',
	'whitealiendragon',
	'yeti',
	'yeti_grass'
]);
const has = (key: string) => INVENTORY.has(key);

describe('resolvePalModelKey', () => {
	it('takes an exact model key as it stands', () => {
		expect(resolvePalModelKey('anubis', has)).toBe('anubis');
	});

	it('matches the manifest case, whatever case the caller holds', () => {
		expect(resolvePalModelKey('Anubis', has)).toBe('anubis');
	});

	// cubeturtle_neutral has a model of its own. Peeling first would render the plain
	// Cube Turtle for it and the variant would never be reachable at all.
	it('prefers a variant that has its own model over the base it derives from', () => {
		expect(resolvePalModelKey('cubeturtle_neutral', has)).toBe('cubeturtle_neutral');
	});

	// Elemental recolors mostly reuse the base skeletal mesh with a material swap, so
	// there is no blackpuppy_ice model to find.
	it('falls back to the base model for a variant with no model of its own', () => {
		expect(resolvePalModelKey('blackpuppy_ice', has)).toBe('blackpuppy');
	});

	it('peels as many variant tokens as it takes', () => {
		expect(resolvePalModelKey('nightlady_dark_2', has)).toBe('nightlady');
	});

	it.each([
		['boss_kingwhale', 'kingwhale'],
		['predator_grimgirl', 'grimgirl'],
		['raid_nightlady', 'nightlady'],
		['summon_whitealiendragon', 'whitealiendragon'],
		['gym_elecpanda', 'elecpanda']
	])('strips the spawn-context prefix on %s', (id, expected) => {
		expect(resolvePalModelKey(id, has)).toBe(expected);
	});

	it.each([
		['BOSS_KingWhale_Otomo', 'kingwhale'],
		['SUMMON_WhiteAlienDragon_MAX', 'whitealiendragon'],
		['RAID_NightLady_Dark_2', 'nightlady'],
		['PREDATOR_Yeti', 'yeti']
	])('resolves %s, prefix and suffix together', (id, expected) => {
		expect(resolvePalModelKey(id, has)).toBe(expected);
	});

	// Peeling "boss_kingwhale" down to "boss" would let any Pal whose model is missing
	// collide with a model that happened to be named after the prefix.
	it('never peels down to a bare prefix token', () => {
		expect(resolvePalModelKey('boss_nothing_here', (key) => key === 'boss')).toBeNull();
	});

	it('strips at most one prefix, so a Pal really named after one survives', () => {
		expect(resolvePalModelKey('raid_boss_thing', (key) => key === 'boss_thing')).toBe('boss_thing');
	});

	it('returns null when no amount of peeling finds a model', () => {
		expect(resolvePalModelKey('blackfurdragon', has)).toBeNull();
	});

	it('returns null for a single token with no model', () => {
		expect(resolvePalModelKey('garm', has)).toBeNull();
	});

	it('returns null for an empty key rather than matching something', () => {
		expect(resolvePalModelKey('', has)).toBeNull();
	});
});
