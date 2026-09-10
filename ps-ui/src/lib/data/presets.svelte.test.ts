import { MessageType, type PresetProfile } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const sendAndWait = vi.fn();
const send = vi.fn();

vi.mock('$lib/utils/websocketUtils', () => ({
	sendAndWait: (type: unknown, data?: unknown) => sendAndWait(type, data),
	send: (type: unknown, data?: unknown) => send(type, data)
}));

import { presetsData } from './presets.svelte';

function profile(id: string): PresetProfile {
	return { id, name: id, type: 'inventory' } as PresetProfile;
}

beforeEach(() => {
	sendAndWait.mockReset();
	send.mockReset();
	presetsData.presetProfiles = { a: profile('a') };
});

describe('presetsData surfaces a failed write to its caller', () => {
	it('rejects when the delete request fails', async () => {
		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await expect(presetsData.removePresetProfiles(['a'])).rejects.toThrow('socket closed');
		expect(sendAndWait).toHaveBeenCalledWith(MessageType.DELETE_PRESET, ['a']);
	});

	it('rejects when the add request fails', async () => {
		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await expect(presetsData.addPresetProfile(profile('b'))).rejects.toThrow('socket closed');
	});

	it('rejects when the clone request fails', async () => {
		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await expect(presetsData.clone('a', 'a copy')).rejects.toThrow('socket closed');
	});

	it('rejects when the rename request fails', async () => {
		send.mockRejectedValueOnce(new Error('socket closed'));
		await expect(presetsData.changePresetName('a', 'renamed')).rejects.toThrow('socket closed');
	});

	it('resolves with the reloaded list when the delete succeeds', async () => {
		sendAndWait.mockResolvedValueOnce(undefined).mockResolvedValueOnce({ b: profile('b') });
		await expect(presetsData.removePresetProfiles(['a'])).resolves.toEqual({ b: profile('b') });
	});
});
