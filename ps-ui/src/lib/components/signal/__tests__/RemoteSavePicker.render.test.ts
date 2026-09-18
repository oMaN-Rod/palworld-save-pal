// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { sendAndWait } = vi.hoisted(() => ({ sendAndWait: vi.fn() }));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: vi.fn(),
	sendAndWait: (type: unknown, data?: unknown) => sendAndWait(type, data)
}));

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

vi.mock('$states', () => ({
	getServerState: () => ({
		servers: [],
		loading: false,
		loadServers: vi.fn(),
		loadServerSave: vi.fn()
	})
}));

import RemoteSavePicker from '../RemoteSavePicker.svelte';

beforeEach(() => {
	sendAndWait.mockReset().mockImplementation(async (type: string) => {
		if (type === 'list_local_saves') {
			return {
				saves: [
					{
						path: 'C:/saves/1/AAA/Level.sav',
						name: 'AAA',
						save_type: 'steam',
						modified_ms: 0,
						world_key: 'C:/saves/1/AAA',
						mod_profile: {
							profile_id: 'client-x/hard',
							profile_name: 'Hard',
							target_id: 'client-x'
						}
					},
					{
						path: 'C:/saves/1/BBB/Level.sav',
						name: 'BBB',
						save_type: 'steam',
						modified_ms: 0,
						world_key: 'C:/saves/1/BBB',
						mod_profile: null
					}
				]
			};
		}
		if (type === 'scan_gamepass_saves') return { saves: {} };
		return { path: '', entries: [] };
	});
});

describe('RemoteSavePicker', () => {
	it('shows the mods profile a world is linked to', async () => {
		render(RemoteSavePicker, { section: 'steam' });
		expect(await screen.findByText('Mods profile: Hard')).toBeTruthy();
		expect(screen.getAllByText(/Mods profile:/)).toHaveLength(1);
	});

	it('drops Game Pass rows from a reply the Worlds tab requested', async () => {
		sendAndWait.mockReset().mockImplementation(async (type: string) => {
			if (type === 'list_local_saves') {
				return {
					saves: [
						{
							path: 'C:/saves/1/AAA/Level.sav',
							name: 'AAA',
							save_type: 'steam',
							modified_ms: 0
						},
						{
							path: 'C:/saves/1/AAA/Level.sav',
							name: 'AAA (Game Pass)',
							save_type: 'gamepass',
							modified_ms: 0
						}
					]
				};
			}
			if (type === 'scan_gamepass_saves') return { saves: {} };
			return { path: '', entries: [] };
		});

		render(RemoteSavePicker, { section: 'steam' });

		expect(await screen.findByText('AAA')).toBeTruthy();
		expect(screen.queryByText('AAA (Game Pass)')).toBeNull();
	});
});
