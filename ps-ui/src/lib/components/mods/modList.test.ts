import type { InstallManifest, ModTarget } from '$types';
import { describe, expect, it } from 'vitest';
import { libraryMod as mod, modTarget, modVersion as version } from './__tests__/fixtures';
import {
	applyOffers,
	deployedText,
	filterMods,
	formatSize,
	holderText,
	initials,
	inUseDeployed,
	inUseFrameworks,
	inUseHolders,
	inUseLines,
	isClientOnlyOn,
	isSubscribed,
	refusedKind,
	releasable,
	sortMods,
	totalSize,
	versionsNewestFirst
} from './modList';

const dockerLayout = {
	ue4ss_mods_dir: null,
	palschema_mods_dir: null,
	paks_mods_dir: '/palworld/paks',
	logicmods_dir: '/palworld/logicmods',
	nativemods_dir: '/palworld/nativemods',
	workshop_local_dir: null,
	mods_txt: null,
	palmodsettings_ini: null
};

function target(overrides: Partial<ModTarget> = {}): ModTarget {
	return modTarget({
		id: 'server-7',
		kind: 'server',
		server_id: 7,
		name: 'Main',
		root_path: '/palworld',
		platform: 'linux',
		layout: dockerLayout,
		...overrides
	});
}

const workshopManifest: InstallManifest = {
	folder_name: 'Pkg',
	display_name: 'Pkg',
	mod_type: 'workshop',
	version: '1',
	routes: [],
	decisions: [],
	platform_filtered: null,
	source: {}
};

const nativeManifest: InstallManifest = {
	...workshopManifest,
	mod_type: 'hybrid',
	routes: [{ archive_path: 'a.dll', rel_path: 'a.dll', kind: 'nativedll' }]
};

describe('formatSize', () => {
	it('uses binary units', () => {
		expect(formatSize(512)).toBe('512 B');
		expect(formatSize(1536)).toBe('1.5 KiB');
		expect(formatSize(5 * 1024 * 1024)).toBe('5.0 MiB');
		expect(formatSize(3 * 1024 ** 3)).toBe('3.0 GiB');
	});

	it('moves to the next unit when rounding reaches it', () => {
		expect(formatSize(1023)).toBe('1023 B');
		expect(formatSize(1048575)).toBe('1.0 MiB');
	});

	it('shows a dash for an unknown size', () => {
		expect(formatSize(null)).toBe('—');
	});
});

describe('totalSize', () => {
	it('sums every version and skips unknown sizes', () => {
		const versions = [version({ size_bytes: 100 }), version({ id: 'v2', size_bytes: null })];
		expect(totalSize(mod({ versions }))).toBe(100);
	});

	it('is unknown when no version has a size', () => {
		expect(totalSize(mod({ versions: [version({ size_bytes: null })] }))).toBeNull();
	});
});

describe('isSubscribed', () => {
	it('needs a workshop source with a digits-only workshop id', () => {
		expect(
			isSubscribed(mod({ source_kind: 'workshop', source_ref: '{"workshop_id":"3301"}' }))
		).toBe(true);
		expect(
			isSubscribed(mod({ source_kind: 'workshop', source_ref: '{"workshop_id":"abc"}' }))
		).toBe(false);
		expect(isSubscribed(mod({ source_kind: 'workshop', source_ref: '{"package":"Pkg"}' }))).toBe(
			false
		);
		expect(isSubscribed(mod({ source_kind: 'local', source_ref: '{"workshop_id":"3301"}' }))).toBe(
			false
		);
		expect(isSubscribed(mod({ source_kind: 'workshop', source_ref: 'not json' }))).toBe(false);
	});
});

describe('isClientOnlyOn', () => {
	it('warns only on a server target whose current version says it is not server capable', () => {
		const clientOnly = mod({
			versions: [version({ manifest: { ...workshopManifest, source: { server_capable: false } } })]
		});
		expect(isClientOnlyOn(target(), clientOnly)).toBe(true);
		expect(isClientOnlyOn(target({ kind: 'client' }), clientOnly)).toBe(false);
	});

	it('says nothing when capability is unrecorded', () => {
		const unknown = mod({ versions: [version({ manifest: workshopManifest })] });
		expect(isClientOnlyOn(target(), unknown)).toBe(false);
	});
});

describe('refusedKind', () => {
	it('refuses a Workshop package on a layout without a Workshop folder', () => {
		const pkg = mod({ mod_type: 'workshop', versions: [version({ manifest: workshopManifest })] });
		expect(refusedKind(target(), pkg)).toBe('workshop');
	});

	it('refuses a native DLL route on a layout without a native mods folder', () => {
		const native = mod({ versions: [version({ manifest: nativeManifest })] });
		const client = target({
			kind: 'client',
			layout: { ...dockerLayout, nativemods_dir: null, workshop_local_dir: '/g/Mods/Workshop' }
		});
		expect(refusedKind(client, native)).toBe('nativedll');
		expect(refusedKind(target(), native)).toBeUndefined();
	});

	it('allows a Workshop package where the layout has a Workshop folder', () => {
		const pkg = mod({ versions: [version({ manifest: workshopManifest })] });
		const withFolder = target({
			layout: { ...dockerLayout, workshop_local_dir: '/palworld/Mods/Workshop' }
		});
		expect(refusedKind(withFolder, pkg)).toBeUndefined();
	});

	it('allows a missing version, an unreadable manifest and an unresolved layout', () => {
		const pkg = mod({ mod_type: 'workshop', versions: [version({ manifest: workshopManifest })] });
		expect(refusedKind(target(), mod({ mod_type: 'workshop' }))).toBeUndefined();
		expect(refusedKind(target(), mod({ current_version_id: null }))).toBeUndefined();
		expect(refusedKind(target({ layout: null }), pkg)).toBeUndefined();
	});

	it('judges the pinned version instead of the current one', () => {
		const pinnedWorkshop = mod({
			versions: [version(), version({ id: 'v2', is_current: false, manifest: workshopManifest })]
		});
		expect(refusedKind(target(), pinnedWorkshop)).toBeUndefined();
		expect(refusedKind(target(), pinnedWorkshop, 'v2')).toBe('workshop');
		expect(refusedKind(target(), pinnedWorkshop, 'gone')).toBeUndefined();
	});
});

describe('inUseLines', () => {
	const labels = {
		target: (id: string) => `T(${id})`,
		profile: (id: string) => `P(${id})`
	};
	const base = { code: 'version_in_use', message: 'mod version v1 is in use' };

	it('names targets and profiles, with none for an empty list', () => {
		expect(inUseLines({ ...base, targets: ['a'], profiles: [] }, labels)).toEqual([
			'In use by targets: T(a) and profiles: none'
		]);
	});

	it('explains the current version, frameworks and open applies', () => {
		expect(
			inUseLines(
				{
					...base,
					is_current: true,
					targets: [],
					profiles: ['p1'],
					frameworks: ['a', 'b'],
					open_applies: ['c']
				},
				labels
			)
		).toEqual([
			'This is the current version. Make another version current before deleting it.',
			'In use by targets: none and profiles: P(p1)',
			'Installed as a framework on: T(a), T(b)',
			'T(c) is applying mods. Try again when it finishes.'
		]);
	});

	it('falls back to the message when no reason is given', () => {
		expect(inUseLines({ ...base, is_current: false, targets: [], profiles: [] }, labels)).toEqual([
			'mod version v1 is in use'
		]);
	});
});

describe('filterMods and sortMods', () => {
	const mods = [
		mod({ id: 'a', name: 'Zeta', author: 'Okaeri', mod_type: 'pak', versions: [version()] }),
		mod({
			id: 'b',
			name: 'Alpha',
			custom_name: 'Better Alpha',
			author: null,
			mod_type: 'ue4ss',
			versions: [version({ size_bytes: 9000 })]
		}),
		mod({ id: 'c', name: 'Mid', author: 'Someone', mod_type: 'logicmods', versions: [] })
	];

	it('matches the display name or the author, ignoring case', () => {
		expect(filterMods(mods, 'okaeri').map((entry) => entry.id)).toEqual(['a']);
		expect(filterMods(mods, 'better').map((entry) => entry.id)).toEqual(['b']);
		expect(filterMods(mods, '  ').map((entry) => entry.id)).toEqual(['a', 'b', 'c']);
	});

	it('sorts by name, type label or size', () => {
		const label = (type: string) => type;
		expect(sortMods(mods, 'name', label).map((entry) => entry.id)).toEqual(['b', 'c', 'a']);
		expect(sortMods(mods, 'type', label).map((entry) => entry.id)).toEqual(['c', 'a', 'b']);
		expect(sortMods(mods, 'size', label).map((entry) => entry.id)).toEqual(['b', 'a', 'c']);
	});
});

describe('initials', () => {
	it.each([
		['Better Pal Stats', 'BP'],
		['alpha', 'A'],
		['[UE4SS] minimap_plus', 'UM'],
		['  ', '?']
	])('abbreviates %j as %j', (name, expected) => {
		expect(initials(name)).toBe(expected);
	});
});

describe('versionsNewestFirst', () => {
	it('orders versions by install time, newest first', () => {
		const older = version({ id: 'old', installed_at: '2026-09-01T10:00:00Z' });
		const newer = version({ id: 'new', installed_at: '2026-09-05T10:00:00Z' });
		expect(versionsNewestFirst(mod({ versions: [older, newer] })).map((entry) => entry.id)).toEqual(
			['new', 'old']
		);
	});
});

describe('structured in-use refusals', () => {
	const labels = {
		target: (id: string) => (id === 'client-abc' ? 'Palworld' : id),
		profile: (id: string) => (id === 'client-abc-default' ? 'Default' : id)
	};
	const error = {
		code: 'version_in_use',
		message: 'in use',
		targets: ['client-abc', 'server-9'],
		profiles: ['client-abc-default', 'server-9-default'],
		holders: [
			{
				target_id: 'client-abc',
				target_name: 'x',
				profile_id: 'client-abc-default',
				profile_name: 'y',
				enabled: true
			},
			{
				target_id: 'server-9',
				target_name: 'Nine',
				profile_id: 'server-9-default',
				profile_name: 'Main',
				enabled: false
			}
		],
		deployed: [
			{ target_id: 'client-abc', target_name: 'x' },
			{ target_id: 'server-9', target_name: 'Nine' }
		]
	};

	it('reads holders and deployments, preferring local labels', () => {
		expect(inUseHolders(error).map((holder) => holderText(holder, labels))).toEqual([
			'Palworld / Default (enabled)',
			'Nine / Main'
		]);
		expect(inUseDeployed(error).map((entry) => deployedText(entry, labels))).toEqual([
			'Palworld',
			'Nine'
		]);
	});

	it('offers a release only with a disabled holder, and an apply only where nothing is enabled', () => {
		expect(releasable(error)).toBe(true);
		expect(applyOffers(error).map((entry) => entry.target_id)).toEqual(['server-9']);
		expect(releasable({ ...error, holders: [error.holders[0]] })).toBe(false);
	});

	it('reads framework slots by local label', () => {
		const frameworks = inUseFrameworks({
			code: 'version_in_use',
			message: '',
			frameworks: [{ target_id: 'client-abc', target_name: 'x' }, 'client-abc', null]
		});
		expect(frameworks.map((entry) => deployedText(entry, labels))).toEqual(['Palworld']);
	});

	it('ignores malformed entries', () => {
		expect(inUseHolders({ code: 'version_in_use', message: '', holders: [null, 3] })).toEqual([]);
		expect(inUseDeployed({ code: 'version_in_use', message: '' })).toEqual([]);
		expect(inUseFrameworks({ code: 'version_in_use', message: '' })).toEqual([]);
	});
});
