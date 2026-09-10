// @vitest-environment jsdom
import { MessageType } from '$types';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import './fixtures/animatePolyfill';

const mockSendAndWait = vi.fn(async (type: string, _data?: unknown) => {
	switch (type) {
		case MessageType.GAME_STATUS:
			return {
				authoritative: true,
				modVersion: '1.0.0',
				mode: 'coop_host',
				protocolVersion: 1,
				queueDepth: 0,
				worldLoaded: true
			};
		case MessageType.GAME_CAPABILITIES:
			return {
				version: 1,
				ops: {
					'pal.heal': { available: true, reason: null },
					'player.edit': { available: true, reason: null },
					'item.setSlot': { available: true, reason: null }
				}
			};
		case MessageType.GAME_PLAYERS:
			return {
				players: [{ uid: 'p1', nickname: 'Aurora', level: 5, exp: 740, status: 'ok' }],
				status: 'ok'
			};
		case MessageType.GET_EXP_DATA:
			return { '1': { TotalEXP: 0 }, '5': { TotalEXP: 740 }, '20': { TotalEXP: 56840 } };
		case MessageType.GAME_PALS:
			return { pals: [], page: 0, pageCount: 0, status: 'ok' };
		case MessageType.GAME_INVENTORY:
			return { playerUid: 'p1', status: 'ok', containers: [] };
		default:
			return {};
	}
});

vi.mock('$utils/websocketUtils', () => ({
	sendAndWait: (...args: [string, unknown?]) => mockSendAndWait(...args),
	send: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

import LivePage from '../../../../routes/live/+page.svelte';

async function openAurora() {
	const user = userEvent.setup();
	render(LivePage);
	await user.click(await screen.findByRole('button', { name: /Aurora/ }));
	return user;
}

beforeEach(() => {
	mockSendAndWait.mockClear();
});

describe('live page level input', () => {
	it('starts at the level the game reported', async () => {
		await openAurora();

		expect(await screen.findByDisplayValue('5')).toBeTruthy();
	});
});

describe('live page level guard', () => {
	it('says why an empty level was refused rather than doing nothing', async () => {
		const user = await openAurora();

		const input = await screen.findByLabelText('Level');
		await user.clear(input);
		await user.click(screen.getByRole('button', { name: 'Set level' }));

		expect(await screen.findByText('Level must be between 1 and 100')).toBeTruthy();
		expect(mockSendAndWait.mock.calls.some(([type]) => type === MessageType.GAME_EDIT_PLAYER)).toBe(
			false
		);
	});
});
