import type { FileRoute, InstallManifest, ModTarget } from '$types';
import { describe, expect, it } from 'vitest';
import { libraryMod, modTarget, modVersion } from './__tests__/fixtures';
import {
	canBindInstances,
	canConvertIostore,
	canLaunch,
	canLinkWorlds,
	hasLegacyPak
} from './modCapabilities';

const client = modTarget();
const server = modTarget({ id: 'server-1', kind: 'server', server_id: 1 });

function route(overrides: Partial<FileRoute> = {}): FileRoute {
	return { archive_path: 'Cool_P.pak', rel_path: 'Cool_P.pak', kind: 'pak', ...overrides };
}

function manifest(routes: FileRoute[]): InstallManifest {
	return {
		folder_name: 'Mod',
		display_name: 'Mod',
		mod_type: 'pak',
		version: '1.0.0',
		routes,
		decisions: [],
		platform_filtered: null,
		source: {}
	};
}

describe('mod capabilities', () => {
	it('launches only a game install from the desktop app outside a remote session', () => {
		expect(canLaunch(client, { desktop: true, remote: false })).toBe(true);
		expect(canLaunch(client, { desktop: true, remote: true })).toBe(false);
		expect(canLaunch(client, { desktop: false, remote: false })).toBe(false);
		expect(canLaunch(server, { desktop: true, remote: false })).toBe(false);
	});

	it('links worlds of a game install in the desktop app or a remote session', () => {
		expect(canLinkWorlds(client, { desktop: true, remote: false })).toBe(true);
		expect(canLinkWorlds(client, { desktop: false, remote: true })).toBe(true);
		expect(canLinkWorlds(client, { desktop: false, remote: false })).toBe(false);
		expect(canLinkWorlds(server, { desktop: true, remote: true })).toBe(false);
	});

	it('binds game instances to targets outside a remote session only', () => {
		expect(canBindInstances({ desktop: true, remote: false })).toBe(true);
		expect(canBindInstances({ desktop: false, remote: false })).toBe(true);
		expect(canBindInstances({ desktop: true, remote: true })).toBe(false);
	});

	describe('hasLegacyPak', () => {
		it('is true for a pak route alone', () => {
			expect(hasLegacyPak(manifest([route()]))).toBe(true);
		});

		it('is true for a passthrough game-tree pak, but not for a path that only contains the prefix', () => {
			expect(
				hasLegacyPak(
					manifest([
						route({
							kind: 'passthrough',
							rel_path: 'Pal/Content/Paks/~mods/Cool_P.pak',
							archive_path: 'Cool_P.pak'
						})
					])
				)
			).toBe(true);
			expect(
				hasLegacyPak(
					manifest([
						route({
							kind: 'passthrough',
							rel_path: 'Pal/Content/Paks/Deep/Pal/Content/Paks/~mods/Cool_P.pak',
							archive_path: 'Cool_P.pak'
						})
					])
				)
			).toBe(false);
		});

		it('is false for a pak with both an .utoc and .ucas sibling', () => {
			expect(
				hasLegacyPak(
					manifest([
						route(),
						route({ rel_path: 'Cool_P.utoc', archive_path: 'Cool_P.utoc' }),
						route({ rel_path: 'Cool_P.ucas', archive_path: 'Cool_P.ucas' })
					])
				)
			).toBe(false);
		});

		it('is true for a pak with only an .utoc sibling', () => {
			expect(
				hasLegacyPak(
					manifest([route(), route({ rel_path: 'Cool_P.utoc', archive_path: 'Cool_P.utoc' })])
				)
			).toBe(true);
		});
	});

	describe('canConvertIostore', () => {
		const gdk = { ...modTarget(), kind: 'client', platform: 'wingdk' } as ModTarget;

		it('offers conversion for a hybrid mod with a pak', () => {
			const hybrid = { ...libraryMod(), mod_type: 'hybrid' };
			const version = modVersion({ manifest: manifest([route()]) });
			expect(canConvertIostore(gdk, { desktop: true, remote: false }, hybrid, version)).toBe(true);
		});

		it('offers no conversion for a ue4ss mod with no pak', () => {
			const ue4ss = { ...libraryMod(), mod_type: 'ue4ss' };
			const version = modVersion({
				manifest: manifest([
					route({ kind: 'ue4ss', rel_path: 'dwmapi.dll', archive_path: 'dwmapi.dll' })
				])
			});
			expect(canConvertIostore(gdk, { desktop: true, remote: false }, ue4ss, version)).toBe(false);
		});

		it('falls back to mod_type when no version manifest is given', () => {
			const pak = { ...libraryMod(), mod_type: 'pak' };
			expect(canConvertIostore(gdk, { desktop: true, remote: false }, pak)).toBe(true);
			const ue4ss = { ...libraryMod(), mod_type: 'ue4ss' };
			expect(canConvertIostore(gdk, { desktop: true, remote: false }, ue4ss)).toBe(false);
		});

		it('is false outside a desktop session, in a remote session, or off wingdk', () => {
			const pak = { ...libraryMod(), mod_type: 'pak' };
			expect(canConvertIostore(gdk, { desktop: true, remote: true }, pak)).toBe(false);
			expect(canConvertIostore(gdk, { desktop: false, remote: false }, pak)).toBe(false);
			expect(
				canConvertIostore({ ...gdk, platform: 'win64' }, { desktop: true, remote: false }, pak)
			).toBe(false);
		});
	});
});
