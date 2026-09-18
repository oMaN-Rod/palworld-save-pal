// @vitest-environment jsdom
import type { Decision, FileRoute, InstallManifest } from '$types';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ManifestReview from '../ManifestReview.svelte';

function manifest(overrides: Partial<InstallManifest> = {}): InstallManifest {
	return {
		folder_name: 'CoolMod',
		display_name: 'Cool Mod',
		mod_type: 'hybrid',
		version: '1.2.0',
		routes: [
			{
				archive_path: 'CoolMod/Scripts/main.lua',
				rel_path: 'CoolMod/Scripts/main.lua',
				kind: 'ue4ss'
			},
			{
				archive_path: 'CoolMod/Scripts/lib/util.lua',
				rel_path: 'CoolMod/Scripts/lib/util.lua',
				kind: 'ue4ss'
			},
			{ archive_path: 'CoolMod/enabled.txt', rel_path: 'CoolMod/enabled.txt', kind: 'ue4ss' },
			{ archive_path: 'Paks/CoolMod_P.pak', rel_path: 'CoolMod_P.pak', kind: 'pak' }
		],
		decisions: [],
		platform_filtered: null,
		source: {},
		...overrides
	};
}

function group(kind: string): HTMLElement {
	const found = document.querySelector(`[data-route-kind="${kind}"]`);
	expect(found).toBeTruthy();
	return found as HTMLElement;
}

describe('ManifestReview', () => {
	it('shows the name, version and mod type', () => {
		render(ManifestReview, { manifest: manifest() });

		expect(screen.getByText('Cool Mod')).toBeTruthy();
		expect(screen.getByText('v1.2.0')).toBeTruthy();
		expect(screen.getByText('Hybrid')).toBeTruthy();
	});

	it('shows no version for an unversioned manifest', () => {
		render(ManifestReview, { manifest: manifest({ version: 'unversioned' }) });

		expect(screen.queryByText(/unversioned/)).toBeNull();
		expect(screen.getByText('Cool Mod')).toBeTruthy();
	});

	it('says when files for other platforms were left out', () => {
		render(ManifestReview, { manifest: manifest({ platform_filtered: 'wingdk' }) });

		expect(
			screen.getByText(/^Files for other platforms were left out; installing the .+ files\.$/)
		).toBeTruthy();
	});

	it('says nothing about platforms when nothing was filtered', () => {
		render(ManifestReview, { manifest: manifest() });

		expect(screen.queryByText(/Files for other platforms/)).toBeNull();
	});

	it('groups routes by kind with a destination label and a count', () => {
		render(ManifestReview, { manifest: manifest() });

		const groups = [...document.querySelectorAll('[data-route-kind]')].map((element) =>
			element.getAttribute('data-route-kind')
		);
		expect(groups).toEqual(['ue4ss', 'pak']);
		expect(within(group('ue4ss')).getByText('UE4SS Mods')).toBeTruthy();
		expect(within(group('ue4ss')).getByText('Files: 3')).toBeTruthy();
		expect(within(group('pak')).getByText('Paks (~mods)')).toBeTruthy();
		expect(within(group('pak')).getByText('Files: 1')).toBeTruthy();
	});

	it.each([
		['logicmods', 'LogicMods'],
		['nativedll', 'NativeMods'],
		['palschema', 'PalSchema'],
		['workshop', 'Workshop'],
		['passthrough', 'Game folder']
	] as const)('labels the %s destination as %s', (kind, label) => {
		render(ManifestReview, {
			manifest: manifest({ routes: [{ archive_path: 'a', rel_path: 'a', kind }] })
		});

		expect(within(group(kind)).getByText(label)).toBeTruthy();
	});

	it('expands a group into nested folders built from rel_path, folders collapsible', async () => {
		render(ManifestReview, { manifest: manifest() });
		const ue4ss = group('ue4ss');
		const toggle = within(ue4ss).getByRole('button', { name: /UE4SS Mods/ });

		expect(toggle.getAttribute('aria-expanded')).toBe('false');
		expect(within(ue4ss).queryByText('main.lua')).toBeNull();

		await fireEvent.click(toggle);

		expect(toggle.getAttribute('aria-expanded')).toBe('true');
		expect(within(ue4ss).getByRole('button', { name: 'CoolMod' })).toBeTruthy();
		expect(within(ue4ss).getByRole('button', { name: 'Scripts' })).toBeTruthy();
		expect(within(ue4ss).getByRole('button', { name: 'lib' })).toBeTruthy();
		expect(within(ue4ss).getByText('util.lua')).toBeTruthy();
		expect(within(ue4ss).getByText('main.lua')).toBeTruthy();
		expect(within(ue4ss).getByText('enabled.txt')).toBeTruthy();
		expect(within(group('pak')).queryByText('CoolMod_P.pak')).toBeNull();

		await fireEvent.click(within(ue4ss).getByRole('button', { name: 'Scripts' }));

		expect(within(ue4ss).queryByText('main.lua')).toBeNull();
		expect(within(ue4ss).queryByText('util.lua')).toBeNull();
		expect(within(ue4ss).getByText('enabled.txt')).toBeTruthy();
	});

	// Rendering 3001 routes is the point of the test, and it lands close enough
	// to the 5s default to time out on a loaded machine.
	it('orders folders before files and merges repeated folders in a large archive', async () => {
		const routes: FileRoute[] = Array.from({ length: 3000 }, (_, index) => ({
			archive_path: `Pack/data/file${index}.json`,
			rel_path: `Pack/data/file${index}.json`,
			kind: 'palschema'
		}));
		routes.push({
			archive_path: 'Pack/readme.txt',
			rel_path: 'Pack/readme.txt',
			kind: 'palschema'
		});
		render(ManifestReview, { manifest: manifest({ routes }) });
		const palschema = group('palschema');

		expect(within(palschema).getByText('Files: 3001')).toBeTruthy();
		await fireEvent.click(within(palschema).getByRole('button', { name: /PalSchema/ }));

		expect(within(palschema).getAllByRole('button', { name: 'data' })).toHaveLength(1);
		const pack = within(palschema)
			.getByRole('button', { name: 'Pack' })
			.closest('li') as HTMLElement;
		const children = [...(pack.querySelector('ul') as HTMLElement).children].map((child) =>
			child.textContent?.trim()
		);
		expect(children[0]?.startsWith('data')).toBe(true);
		expect(children.at(-1)).toBe('readme.txt');
	}, 20_000);

	it.each<[Decision, string]>([
		[
			{ kind: 'multiple_ue4ss_roots', roots: ['AlphaMod', 'BetaMod'] },
			'This archive contains more than one UE4SS mod: AlphaMod, BetaMod. They will be installed together.'
		],
		[
			{ kind: 'pak_destination', file: 'CoolMod_P.pak', default: 'pak' },
			'CoolMod_P.pak will go to Paks (~mods).'
		],
		[
			{ kind: 'unplaced_files', files: ['readme.md', 'extra.bin'] },
			'These files have no known destination and will be skipped: readme.md, extra.bin'
		],
		[
			{ kind: 'name_conflict', proposed: 'Cool Mod', existing_mod_id: 'cool-mod' },
			'A mod named Cool Mod is already in your library.'
		],
		[
			{ kind: 'nexus_variant', existing_mod_id: 'cool-mod', file_name: 'CoolMod-2.zip' },
			'This looks like another variant of a mod you already have.'
		]
	])('renders a %o decision as a sentence', (decision, sentence) => {
		render(ManifestReview, { manifest: manifest(), decisions: [decision] });

		const list = screen.getByRole('list', { name: 'Check before installing' });
		expect(within(list).getByText(sentence)).toBeTruthy();
	});

	it('renders no warning list with an empty or missing decision list', () => {
		render(ManifestReview, { manifest: manifest(), decisions: [] });
		expect(screen.getByText('Cool Mod')).toBeTruthy();
		expect(screen.queryByRole('list', { name: 'Check before installing' })).toBeNull();
		cleanup();

		render(ManifestReview, { manifest: manifest() });
		expect(screen.getByText('Cool Mod')).toBeTruthy();
		expect(screen.queryByRole('list', { name: 'Check before installing' })).toBeNull();
	});

	it('says when nothing in the archive will be installed', () => {
		render(ManifestReview, { manifest: manifest({ routes: [] }) });

		expect(screen.getByText('Nothing in this archive has a place to go.')).toBeTruthy();
		expect(document.querySelector('[data-route-kind]')).toBeNull();
	});
});
