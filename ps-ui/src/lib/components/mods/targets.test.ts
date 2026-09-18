import type { ModTarget } from '$types';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$i18n/messages', () => ({
	mods_hazard_ue4ss_dual_instance: () => 'UE4SS twice',
	mods_hazard_workshop_proxy_dll: () => 'Proxy DLL beside Workshop UE4SS',
	mods_hazard_amity_legacy_folder: () => 'Old PSPAmity folder',
	mods_target_remove_title: () => 'Remove game install',
	mods_target_remove_message: ({ name }: { name: string }) => `Stop managing ${name}`,
	mods_target_remove: () => 'Remove',
	mods_panel_cancel: () => 'Cancel',
	mods_platform_win64: () => 'Windows',
	mods_platform_wingdk: () => 'Windows (Xbox app)',
	mods_platform_linux: () => 'Linux',
	mods_platform_mac: () => 'macOS'
}));

import { modTarget } from './__tests__/fixtures';
import {
	confirmRemoveTarget,
	hazardText,
	normaliseRoot,
	platformLabel,
	targetHazards,
	targetName
} from './targets';

describe('platformLabel', () => {
	it('names a known platform and shows an unknown one as its code', () => {
		expect(platformLabel('wingdk')).toBe('Windows (Xbox app)');
		expect(platformLabel('amiga')).toBe('amiga');
	});
});

function target(overrides: Partial<ModTarget> = {}): ModTarget {
	return modTarget({ root_path: 'C:/Games/Palworld', platform: 'steam', ...overrides });
}

describe('normaliseRoot', () => {
	it('matches a backslashed root against a forward-slashed one of different case', () => {
		expect(normaliseRoot('C:\\Games\\Palworld')).toBe(normaliseRoot('c:/games/palworld'));
	});

	it('ignores a trailing separator', () => {
		expect(normaliseRoot('C:\\Games\\Palworld\\')).toBe(normaliseRoot('C:/Games/Palworld'));
	});
});

describe('hazardText', () => {
	it('describes a known hazard in words', () => {
		expect(hazardText('ue4ss_dual_instance')).toBe('UE4SS twice');
	});

	it('shows an unknown hazard code as is', () => {
		expect(hazardText('something_new')).toBe('something_new');
	});
});

describe('targetHazards', () => {
	it('is empty when nothing was detected', () => {
		expect(targetHazards(target())).toEqual([]);
		expect(targetHazards(target({ detected: {} }))).toEqual([]);
	});
});

describe('targetName', () => {
	const servers = [{ id: 7, name: 'Main World' }];

	it('names a server target after its server', () => {
		expect(targetName(target({ kind: 'server', server_id: 7, name: 'main' }), servers)).toBe(
			'Main World'
		);
	});

	it('falls back to the stored name when the server is not loaded', () => {
		expect(targetName(target({ kind: 'server', server_id: 9, name: 'main' }), servers)).toBe(
			'main'
		);
	});

	it('keeps a client target name', () => {
		expect(targetName(target({ server_id: 7 }), servers)).toBe('Palworld');
	});
});

describe('confirmRemoveTarget', () => {
	it('removes a client target once confirmed', async () => {
		const modal = { showConfirmModal: vi.fn().mockResolvedValue(true) };
		const mods = { removeTarget: vi.fn() };

		await confirmRemoveTarget(target(), 'Palworld', modal, mods);

		expect(modal.showConfirmModal).toHaveBeenCalledWith(
			expect.objectContaining({ message: 'Stop managing Palworld' })
		);
		expect(mods.removeTarget).toHaveBeenCalledWith('client-abc');
	});

	it('sends nothing when the confirmation is declined', async () => {
		const modal = { showConfirmModal: vi.fn().mockResolvedValue(false) };
		const mods = { removeTarget: vi.fn() };

		await confirmRemoveTarget(target(), 'Palworld', modal, mods);

		expect(mods.removeTarget).not.toHaveBeenCalled();
	});

	it('never offers to remove a server target', async () => {
		const modal = { showConfirmModal: vi.fn().mockResolvedValue(true) };
		const mods = { removeTarget: vi.fn() };

		await confirmRemoveTarget(target({ kind: 'server', server_id: 7 }), 'Main', modal, mods);

		expect(modal.showConfirmModal).not.toHaveBeenCalled();
		expect(mods.removeTarget).not.toHaveBeenCalled();
	});
});
