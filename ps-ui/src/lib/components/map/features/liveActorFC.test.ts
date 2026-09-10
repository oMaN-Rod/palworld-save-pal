import { expect, test } from 'vitest';
import { buildLiveActorFC } from './features';
import { ICON_LIVE_PAL, ICON_LIVE_PLAYER, livePalIconId } from '../style/iconIds';

const player = {
	id: 'p1',
	kind: 'player',
	x: -201.1,
	y: -138238.3,
	z: 2916.1,
	yaw: 26.3,
	name: 'A'
};

test('places actors in MainMap and drops off-area ones', () => {
	const fc = buildLiveActorFC(
		[player, { id: 'x', kind: 'wild', x: 9_999_999, y: 9_999_999, z: 0 }],
		'MainMap'
	);
	expect(fc.features).toHaveLength(1);
	expect(fc.features[0].properties.key).toBe('p1');
	expect(fc.features[0].properties.featureType).toBe('live_player');
	const [lng, lat] = fc.features[0].geometry.coordinates;
	expect(Number.isFinite(lng) && Number.isFinite(lat)).toBe(true);
});

test('maps palbox and other pal kinds to their feature types', () => {
	const fc = buildLiveActorFC(
		[
			{ id: 'box1', kind: 'palbox', x: -201.1, y: -138238.3, z: 0 },
			{ id: 'otomo1', kind: 'otomo', x: -201.1, y: -138238.3, z: 0 }
		],
		'MainMap'
	);
	expect(fc.features.map((f) => f.properties.featureType)).toEqual(['live_palbox', 'live_pal']);
});

test('gives a pal with a species its bordered live icon id', () => {
	const fc = buildLiveActorFC(
		[{ id: 'otomo1', kind: 'otomo', x: -201.1, y: -138238.3, z: 0, species: 'JetDragon' }],
		'MainMap'
	);
	expect(fc.features[0].properties.icon).toBe(livePalIconId('JetDragon'));
	expect(fc.features[0].properties.species).toBe('JetDragon');
});

test('falls back to the directional marker when a pal has no species', () => {
	const fc = buildLiveActorFC(
		[{ id: 'wild1', kind: 'wild', x: -201.1, y: -138238.3, z: 0 }],
		'MainMap'
	);
	expect(fc.features[0].properties.icon).toBe(ICON_LIVE_PAL);
	expect(fc.features[0].properties.species).toBe('');
});

test('keeps player and palbox icons unaffected by species', () => {
	const fc = buildLiveActorFC(
		[
			{ ...player, species: 'JetDragon' },
			{ id: 'box1', kind: 'palbox', x: -201.1, y: -138238.3, z: 0, species: 'JetDragon' }
		],
		'MainMap'
	);
	expect(fc.features[0].properties.icon).toBe(ICON_LIVE_PLAYER);
	expect(fc.features[1].properties.icon).not.toBe(livePalIconId('JetDragon'));
});
