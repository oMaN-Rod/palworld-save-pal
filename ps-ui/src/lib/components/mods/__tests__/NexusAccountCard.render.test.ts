// @vitest-environment jsdom
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send, modal, env } = vi.hoisted(() => ({
	holder: { mods: undefined as unknown, nexus: undefined as unknown },
	send: vi.fn(),
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	env: { desktop: 'true' }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$env/static/public', () => ({
	get PUBLIC_DESKTOP_MODE() {
		return env.desktop;
	}
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	const { NexusState } = await import('$lib/states/nexusState.svelte');
	holder.mods = new ModsState();
	holder.nexus = new NexusState();
	return {
		getModsState: () => holder.mods,
		getNexusState: () => holder.nexus,
		getModalState: () => modal,
		getToastState: () => ({ add: vi.fn() })
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import type { NexusState } from '$lib/states/nexusState.svelte';
import {
	nexusAccountGetHandler,
	nexusHandlerRegisterHandler,
	nexusHandlerStatusHandler,
	nexusKeySetHandler
} from '$lib/ws/handlers/nexusHandler';
import NexusAccountCard from '../NexusAccountCard.svelte';
import NexusKeyModal from '../NexusKeyModal.svelte';

const context = {} as never;
const modsState = () => holder.mods as ModsState;
const nexusState = () => holder.nexus as NexusState;

function sent(type: MessageType) {
	return send.mock.calls.filter((call) => call[0] === type);
}

beforeEach(() => {
	send.mockReset();
	modal.showModal.mockReset();
	modal.showConfirmModal.mockReset();
	env.desktop = 'true';
	modsState().reset();
	nexusState().reset();
});

describe('NexusAccountCard', () => {
	it('shows the anonymous sentence and an Add API key button with no key stored', () => {
		render(NexusAccountCard);

		expect(
			screen.getByText(
				'Browsing anonymously. Add your API key to download files and see your account.'
			)
		).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Add API key' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Replace API key' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Remove API key' })).toBeNull();
	});

	it('shows the account name, the Premium badge and a profile link for a premium account', async () => {
		render(NexusAccountCard);
		await nexusAccountGetHandler.handle(
			{
				has_key: true,
				account: {
					user_id: 7,
					name: 'Tester',
					is_premium: true,
					is_supporter: false,
					profile_url: 'https://www.nexusmods.com/users/7'
				},
				rate_limit: null
			},
			context
		);
		await tick();

		expect(screen.getByText('Tester')).toBeTruthy();
		expect(screen.getByText('Premium')).toBeTruthy();
		const link = screen.getByRole('link', { name: 'Open the Nexus Mods profile for Tester' });
		expect(link.getAttribute('href')).toBe('https://www.nexusmods.com/users/7');
	});

	it('renders no link at all for a javascript: profile_url, but still renders the name', async () => {
		render(NexusAccountCard);
		await nexusAccountGetHandler.handle(
			{
				has_key: true,
				account: {
					user_id: 7,
					name: 'Tester',
					is_premium: true,
					is_supporter: false,
					profile_url: 'javascript:alert(1)'
				},
				rate_limit: null
			},
			context
		);
		await tick();

		expect(screen.getByText('Tester')).toBeTruthy();
		expect(document.querySelector('a[href^="javascript:"]')).toBeNull();
		expect(screen.queryByRole('link')).toBeNull();
	});

	it('renders the other program by name for a foreign handler, and sends force:true after confirming', async () => {
		render(NexusAccountCard);
		await nexusHandlerStatusHandler.handle(
			{
				supported: true,
				registered: false,
				foreign: true,
				current: '"C:\\Vortex\\Vortex.exe" -d "%1"'
			},
			context
		);
		await tick();

		expect(screen.getByText(/Vortex\.exe/)).toBeTruthy();

		modal.showConfirmModal.mockResolvedValue(true);
		await fireEvent.click(screen.getByRole('button', { name: 'Replace the other program' }));
		await vi.waitFor(() => expect(modal.showConfirmModal).toHaveBeenCalledTimes(1));

		expect(sent(MessageType.NEXUS_HANDLER_REGISTER)).toHaveLength(1);
		expect(sent(MessageType.NEXUS_HANDLER_REGISTER)[0][1]).toEqual({ force: true });
	});

	it('shows the unsupported sentence and no register button on an unsupported platform', async () => {
		render(NexusAccountCard);
		await nexusHandlerStatusHandler.handle(
			{ supported: false, registered: false, foreign: false, current: null },
			context
		);
		await tick();

		expect(screen.getByText('This platform cannot register link handlers yet.')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Let PalStudio open these links' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Replace the other program' })).toBeNull();
	});

	it('renders a register_failed refusal from the register button, not only a foreign_handler one', async () => {
		render(NexusAccountCard);
		await nexusHandlerStatusHandler.handle(
			{ supported: true, registered: false, foreign: false, current: null },
			context
		);
		await tick();

		await fireEvent.click(screen.getByRole('button', { name: 'Let PalStudio open these links' }));
		await nexusHandlerRegisterHandler.handle(
			{ error: { code: 'register_failed', message: 'registering the link handler failed' } },
			context
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('Registering the link handler failed.');
	});

	it('confirms before clearing the stored key, and only sends nexus_key_clear once confirmed', async () => {
		render(NexusAccountCard);
		await nexusAccountGetHandler.handle(
			{
				has_key: true,
				account: {
					user_id: 7,
					name: 'Tester',
					is_premium: true,
					is_supporter: false,
					profile_url: null
				},
				rate_limit: null
			},
			context
		);
		await tick();

		modal.showConfirmModal.mockResolvedValue(false);
		await fireEvent.click(screen.getByRole('button', { name: 'Remove API key' }));
		await vi.waitFor(() => expect(modal.showConfirmModal).toHaveBeenCalledTimes(1));
		expect(sent(MessageType.NEXUS_KEY_CLEAR)).toHaveLength(0);

		modal.showConfirmModal.mockResolvedValue(true);
		await fireEvent.click(screen.getByRole('button', { name: 'Remove API key' }));
		await vi.waitFor(() => expect(sent(MessageType.NEXUS_KEY_CLEAR)).toHaveLength(1));
	});
});

describe('NexusKeyModal', () => {
	it('sends nexus_key_set with the typed key and clears the field afterwards', async () => {
		render(NexusKeyModal, { closeModal: vi.fn() });

		const input = screen.getByLabelText('API key') as HTMLInputElement;
		await fireEvent.input(input, { target: { value: 'my-secret-token' } });
		await fireEvent.click(screen.getByRole('button', { name: 'Save key' }));

		expect(sent(MessageType.NEXUS_KEY_SET)).toHaveLength(1);
		expect(sent(MessageType.NEXUS_KEY_SET)[0][1]).toEqual({ key: 'my-secret-token' });
		expect(input.value).toBe('');
	});

	it('marks the field as password, non-autocompleting and unspellchecked', () => {
		render(NexusKeyModal, { closeModal: vi.fn() });

		const input = screen.getByLabelText('API key') as HTMLInputElement;
		expect(input.type).toBe('password');
		expect(input.getAttribute('autocomplete')).toBe('off');
		expect(input.getAttribute('spellcheck')).toBe('false');
	});

	it('renders an invalid_key refusal without repopulating the field', async () => {
		render(NexusKeyModal, { closeModal: vi.fn() });

		const input = screen.getByLabelText('API key') as HTMLInputElement;
		await fireEvent.input(input, { target: { value: 'not-a-real-key' } });
		await fireEvent.click(screen.getByRole('button', { name: 'Save key' }));

		await nexusKeySetHandler.handle(
			{ error: { code: 'invalid_key', message: 'that is not a Nexus Mods API key' } },
			context
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('did not accept');
		expect(input.value).toBe('');
	});

	it('moves focus into the field on open', async () => {
		render(NexusKeyModal, { closeModal: vi.fn() });
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(document.activeElement).toBe(screen.getByLabelText('API key'));
	});

	it('closes on a successful key set, not just on Cancel', async () => {
		const closeModal = vi.fn();
		render(NexusKeyModal, { closeModal });

		const input = screen.getByLabelText('API key') as HTMLInputElement;
		await fireEvent.input(input, { target: { value: 'my-secret-token' } });
		await fireEvent.click(screen.getByRole('button', { name: 'Save key' }));
		expect(closeModal).not.toHaveBeenCalled();

		await nexusKeySetHandler.handle(
			{
				has_key: true,
				account: {
					user_id: 7,
					name: 'Tester',
					is_premium: true,
					is_supporter: false,
					profile_url: null
				},
				rate_limit: null
			},
			context
		);
		await tick();

		expect(closeModal).toHaveBeenCalledTimes(1);
	});
});
