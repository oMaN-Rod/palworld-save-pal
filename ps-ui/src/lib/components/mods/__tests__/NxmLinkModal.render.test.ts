// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send } = vi.hoisted(() => ({
	holder: { mods: undefined as unknown, nexus: undefined as unknown },
	send: vi.fn()
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	const { NexusState } = await import('$lib/states/nexusState.svelte');
	holder.mods = new ModsState();
	holder.nexus = new NexusState();
	return {
		getModsState: () => holder.mods,
		getNexusState: () => holder.nexus
	};
});

import { MessageType } from '$types';
import type { ModsState } from '$lib/states/modsState.svelte';
import type { NexusState } from '$lib/states/nexusState.svelte';
import { modTarget } from './fixtures';
import NxmLinkModal from '../NxmLinkModal.svelte';

const modsState = () => holder.mods as ModsState;
const nexusState = () => holder.nexus as NexusState;

function sent(type: MessageType) {
	return send.mock.calls.filter((call) => call[0] === type);
}

beforeEach(() => {
	send.mockReset();
	modsState().reset();
	nexusState().reset();
});

describe('NxmLinkModal', () => {
	it('renders the mod and file numbers, and downloads into the chosen target with key and expires', async () => {
		modsState().targets = [modTarget({ id: 'client-a', name: 'Install A' })];
		const closeModal = vi.fn();
		render(NxmLinkModal, {
			payload: { link: { mod_id: 4821, file_id: 99001, key: 'secret-key', expires: 4102444800 } },
			closeModal
		});

		expect(screen.getByText(/4821/)).toBeTruthy();
		expect(screen.getByText(/99001/)).toBeTruthy();

		await fireEvent.click(screen.getByRole('button', { name: 'Download' }));

		expect(sent(MessageType.NEXUS_DOWNLOAD)).toHaveLength(1);
		expect(sent(MessageType.NEXUS_DOWNLOAD)[0][1]).toEqual({
			target_id: 'client-a',
			mod_id: 4821,
			file_id: 99001,
			key: 'secret-key',
			expires: 4102444800
		});
		expect(closeModal).toHaveBeenCalledTimes(1);
	});

	it('sends neither key nor expires when only key is present', async () => {
		modsState().targets = [modTarget({ id: 'client-a', name: 'Install A' })];
		render(NxmLinkModal, {
			payload: { link: { mod_id: 4821, file_id: 99001, key: 'secret-key', expires: null } },
			closeModal: vi.fn()
		});

		await fireEvent.click(screen.getByRole('button', { name: 'Download' }));

		expect(sent(MessageType.NEXUS_DOWNLOAD)).toHaveLength(1);
		expect(sent(MessageType.NEXUS_DOWNLOAD)[0][1]).toEqual({
			target_id: 'client-a',
			mod_id: 4821,
			file_id: 99001,
			key: undefined,
			expires: undefined
		});
	});

	it('never renders the link key anywhere in the DOM', () => {
		modsState().targets = [modTarget({ id: 'client-a', name: 'Install A' })];
		const { container } = render(NxmLinkModal, {
			payload: { link: { mod_id: 4821, file_id: 99001, key: 'secret-key', expires: 4102444800 } },
			closeModal: vi.fn()
		});

		expect(container.innerHTML).not.toContain('secret-key');
	});

	it('shows the expired sentence and no download button for a parsed-but-expired link', () => {
		modsState().targets = [modTarget({ id: 'client-a', name: 'Install A' })];
		render(NxmLinkModal, {
			payload: {
				link: { mod_id: 4821, file_id: 99001, key: 'secret-key', expires: 1 },
				error: { code: 'link_expired', message: 'this download link has expired' }
			},
			closeModal: vi.fn()
		});

		expect(
			screen.getByText('That download link has expired. Start the download again on the Nexus Mods site.')
		).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Download' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Dismiss' })).toBeTruthy();
	});

	it('shows the unsupported sentence and no download button for an unsupported_link rejection', () => {
		modsState().targets = [modTarget({ id: 'client-a', name: 'Install A' })];
		render(NxmLinkModal, {
			payload: { error: { code: 'unsupported_link', message: 'not a Palworld mod' } },
			closeModal: vi.fn()
		});

		expect(screen.getByText('That link is not a Palworld mod download.')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Download' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Dismiss' })).toBeTruthy();
	});

	it('shows the generic invalid sentence and no download button for any other rejection', () => {
		modsState().targets = [modTarget({ id: 'client-a', name: 'Install A' })];
		render(NxmLinkModal, {
			payload: { error: { code: 'invalid_link', message: 'could not parse' } },
			closeModal: vi.fn()
		});

		expect(screen.getByText('That link could not be read.')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Download' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Dismiss' })).toBeTruthy();
	});

	it('offers only Dismiss when no game install exists', () => {
		render(NxmLinkModal, {
			payload: { link: { mod_id: 4821, file_id: 99001, key: 'secret-key', expires: 4102444800 } },
			closeModal: vi.fn()
		});

		expect(screen.getByText('Add a game install before downloading mods.')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Download' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Dismiss' })).toBeTruthy();
	});
});
