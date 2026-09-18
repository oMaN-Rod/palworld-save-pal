import { describe, expect, it } from 'vitest';
import { updateAction } from './nexusUpdates';
import type { ModUpdate } from '$types';

function update(overrides: Partial<ModUpdate> = {}): ModUpdate {
	return {
		mod_id: 'nexus-4821',
		nexus_mod_id: 4821,
		installed_version: '1.2.0',
		latest: {
			file_id: 99002,
			name: 'Enhanced Visuals',
			version: '1.3.0',
			category: 'MAIN',
			date: 0,
			size_in_bytes: null,
			uri: 'file.zip',
			primary: true,
			description: null
		},
		state: 'available',
		ignored_version: null,
		...overrides
	};
}

const desktop = { desktop: true, remote: false };

describe('updateAction', () => {
	it('offers an update when one is available on the desktop', () => {
		expect(updateAction(update(), desktop)).toEqual({ kind: 'update', version: '1.3.0', fileId: 99002 });
	});

	it('offers nothing when the mod is up to date or has no latest file', () => {
		expect(updateAction(update({ state: 'up_to_date' }), desktop).kind).toBe('none');
		expect(updateAction(update({ latest: null }), desktop).kind).toBe('none');
	});

	it('reports an ignored version instead of an update', () => {
		expect(updateAction(update({ state: 'ignored', ignored_version: '1.3.0' }), desktop)).toEqual({
			kind: 'ignored',
			version: '1.3.0'
		});
	});

	it('offers nothing at all in a remote session or outside the desktop app', () => {
		expect(updateAction(update(), { desktop: true, remote: true }).kind).toBe('none');
		expect(updateAction(update(), { desktop: false, remote: false }).kind).toBe('none');
	});

	it('offers nothing without an update record', () => {
		expect(updateAction(undefined, desktop).kind).toBe('none');
	});
});
