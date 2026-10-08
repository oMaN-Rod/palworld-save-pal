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
	nexusSignInHandler
} from '$lib/ws/handlers/nexusHandler';
import NexusAccountCard from '../NexusAccountCard.svelte';

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
	it('shows the anonymous sentence and a sign-in button while signed out', () => {
		render(NexusAccountCard);

		expect(
			screen.getByText('Browsing anonymously. Sign in to download files and see your account.')
		).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Sign in with Nexus Mods' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Sign out' })).toBeNull();
	});

	it('shows the account name, the Premium badge and a profile link for a premium account', async () => {
		render(NexusAccountCard);
		await nexusAccountGetHandler.handle(
			{
				signed_in: true,
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
				signed_in: true,
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

	it('confirms before signing out, and only sends nexus_sign_out once confirmed', async () => {
		render(NexusAccountCard);
		await nexusAccountGetHandler.handle(
			{
				signed_in: true,
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
		await fireEvent.click(screen.getByRole('button', { name: 'Sign out' }));
		await vi.waitFor(() => expect(modal.showConfirmModal).toHaveBeenCalledTimes(1));
		expect(sent(MessageType.NEXUS_SIGN_OUT)).toHaveLength(0);

		modal.showConfirmModal.mockResolvedValue(true);
		await fireEvent.click(screen.getByRole('button', { name: 'Sign out' }));
		await vi.waitFor(() => expect(sent(MessageType.NEXUS_SIGN_OUT)).toHaveLength(1));
	});
});

describe('NexusAccountCard sign-in', () => {
	it('sends nexus_sign_in and waits with a cancel button until Nexus Mods answers', async () => {
		render(NexusAccountCard);

		await fireEvent.click(screen.getByRole('button', { name: 'Sign in with Nexus Mods' }));

		expect(sent(MessageType.NEXUS_SIGN_IN)).toHaveLength(1);
		expect(screen.getByRole('status').textContent).toContain('Approve PalStudio');
		expect(screen.queryByRole('button', { name: 'Sign in with Nexus Mods' })).toBeNull();

		await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(sent(MessageType.NEXUS_SIGN_IN_CANCEL)).toHaveLength(1);

		await nexusSignInHandler.handle(
			{ error: { code: 'sign_in_cancelled', message: 'cancelled' } },
			context
		);
		await tick();

		expect(screen.getByRole('button', { name: 'Sign in with Nexus Mods' })).toBeTruthy();
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('shows the account once the sign-in finishes', async () => {
		render(NexusAccountCard);
		await fireEvent.click(screen.getByRole('button', { name: 'Sign in with Nexus Mods' }));

		await nexusSignInHandler.handle(
			{
				signed_in: true,
				account: {
					user_id: 7,
					name: 'Tester',
					is_premium: false,
					is_supporter: false,
					profile_url: null
				},
				rate_limit: null
			},
			context
		);
		await tick();

		expect(screen.getByText('Tester')).toBeTruthy();
		expect(screen.getByText('Free account')).toBeTruthy();
		expect(screen.queryByRole('status')).toBeNull();
	});

	it('drops the expiry warning once the user has signed in again', async () => {
		render(NexusAccountCard);
		await nexusAccountGetHandler.handle(
			{ signed_in: false, error: { code: 'sign_in_expired', message: 'expired' } },
			context
		);
		await tick();
		expect(screen.getByRole('alert').textContent).toContain('has expired');

		await fireEvent.click(screen.getByRole('button', { name: 'Sign in with Nexus Mods' }));
		await nexusSignInHandler.handle(
			{
				signed_in: true,
				account: {
					user_id: 7,
					name: 'Tester',
					is_premium: false,
					is_supporter: false,
					profile_url: null
				},
				rate_limit: null
			},
			context
		);
		await tick();

		expect(screen.getByText('Tester')).toBeTruthy();
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('offers a retry, not another sign-in, when the account could not be read', async () => {
		render(NexusAccountCard);
		await fireEvent.click(screen.getByRole('button', { name: 'Sign in with Nexus Mods' }));
		await nexusSignInHandler.handle(
			{ signed_in: true, error: { code: 'network', message: 'no route' } },
			context
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('could not be reached');
		expect(screen.queryByRole('button', { name: 'Sign in with Nexus Mods' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Sign out' })).toBeTruthy();

		const before = sent(MessageType.NEXUS_ACCOUNT_GET).length;
		await fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
		expect(sent(MessageType.NEXUS_ACCOUNT_GET)).toHaveLength(before + 1);

		await nexusAccountGetHandler.handle(
			{
				signed_in: true,
				account: {
					user_id: 7,
					name: 'Tester',
					is_premium: false,
					is_supporter: false,
					profile_url: null
				},
				rate_limit: null
			},
			context
		);
		await tick();

		expect(screen.getByText('Tester')).toBeTruthy();
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('renders a failed sign-in under the button and clears it on the next try', async () => {
		render(NexusAccountCard);
		await fireEvent.click(screen.getByRole('button', { name: 'Sign in with Nexus Mods' }));
		await nexusSignInHandler.handle(
			{ error: { code: 'sign_in_timed_out', message: 'too slow' } },
			context
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('took too long');

		await fireEvent.click(screen.getByRole('button', { name: 'Sign in with Nexus Mods' }));
		expect(screen.queryByRole('alert')).toBeNull();
		expect(sent(MessageType.NEXUS_SIGN_IN)).toHaveLength(2);
	});
});
