// @vitest-environment jsdom
import type { DetectedInstall, ModTarget } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send, env, toast } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	send: vi.fn(),
	env: { desktop: 'false' },
	toast: { add: vi.fn() }
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
	holder.state = new ModsState();
	return {
		getModsState: () => holder.state,
		getToastState: () => toast
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import {
	modTargetAddHandler,
	modTargetDetectHandler,
	modTargetListHandler
} from '$lib/ws/handlers/modsHandler';
import AddTargetModal from '../AddTargetModal.svelte';
import { modTarget } from './fixtures';

const state = () => holder.state as ModsState;
const context = { goto: vi.fn() } as never;

function install(root: string, hazards: string[] = []): DetectedInstall {
	return { root, platform: 'win64', ue4ss_mode: 'none', hazards, source: 'steam' };
}

function target(id: string, root_path: string): ModTarget {
	return modTarget({ id, root_path });
}

async function detected(installs: DetectedInstall[]): Promise<void> {
	await modTargetDetectHandler.handle({ detected: installs }, context);
	await tick();
}

beforeEach(() => {
	send.mockReset();
	toast.add.mockReset();
	env.desktop = 'false';
	state().reset();
	state().targets = [target('client-games', 'c:/games/palworld')];
});

describe('AddTargetModal', () => {
	it('detects installs on open, hiding a stale list while it looks', async () => {
		state().detected = [install('Z:\\Old\\Palworld')];
		render(AddTargetModal, { closeModal: vi.fn() });

		expect(send).toHaveBeenCalledWith(MessageType.MOD_TARGET_DETECT, undefined);
		expect(screen.getByText('Looking for installs…')).toBeTruthy();
		expect(screen.queryByText('Z:\\Old\\Palworld')).toBeNull();
		expect(screen.queryByText('No Palworld installs were found automatically.')).toBeNull();

		await detected([]);
		expect(screen.queryByText('Looking for installs…')).toBeNull();
		expect(screen.getByText('No Palworld installs were found automatically.')).toBeTruthy();
	});

	it('marks a detected install already registered under another spelling as added', async () => {
		env.desktop = 'true';
		render(AddTargetModal, { closeModal: vi.fn() });
		await detected([install('C:\\Games\\Palworld'), install('D:\\Steam\\Palworld')]);

		expect(screen.getAllByText('Added')).toHaveLength(1);
		expect(screen.getAllByRole('button', { name: 'Add' })).toHaveLength(1);
		expect(screen.getAllByText('Windows')).toHaveLength(2);
	});

	it('describes a known hazard in words and shows an unknown one as its code', async () => {
		render(AddTargetModal, { closeModal: vi.fn() });
		await detected([install('D:\\Steam\\Palworld', ['ue4ss_dual_instance', 'new_hazard'])]);

		expect(screen.getByText(/Running both crashes the game/)).toBeTruthy();
		expect(screen.getByText('new_hazard')).toBeTruthy();
	});

	it('closes with the new target once its add succeeds', async () => {
		env.desktop = 'true';
		const closeModal = vi.fn();
		render(AddTargetModal, { closeModal });
		await detected([install('D:\\Steam\\Palworld')]);

		await fireEvent.click(screen.getByRole('button', { name: 'Add' }));
		expect(send).toHaveBeenCalledWith(MessageType.MOD_TARGET_ADD, {
			root_path: 'D:\\Steam\\Palworld'
		});

		await modTargetAddHandler.handle(
			{ target: target('client-steam', 'D:/Steam/Palworld') },
			context
		);
		await tick();
		expect(closeModal).toHaveBeenCalledWith('client-steam');
	});

	it('stays open when a target list lands during a pick that is then cancelled', async () => {
		env.desktop = 'true';
		const closeModal = vi.fn();
		render(AddTargetModal, { closeModal });

		await fireEvent.click(screen.getByRole('button', { name: /Choose folder/ }));
		expect(send).toHaveBeenCalledWith(MessageType.MOD_TARGET_ADD, { root_path: '__select__' });

		await modTargetListHandler.handle(
			{ targets: [target('client-games', 'c:/games/palworld'), target('client-x', 'E:/X')] },
			context
		);
		await tick();
		await modTargetAddHandler.handle({ root_path: '__select__', canceled: true }, context);
		await tick();

		expect(closeModal).not.toHaveBeenCalled();
		expect(screen.getByRole('button', { name: /Choose folder/ }).hasAttribute('disabled')).toBe(
			false
		);
	});

	it('stays open on a refused pick, shows the message and releases the button', async () => {
		env.desktop = 'true';
		const closeModal = vi.fn();
		render(AddTargetModal, { closeModal });

		await fireEvent.click(screen.getByRole('button', { name: /Choose folder/ }));
		await modTargetAddHandler.handle(
			{
				root_path: 'E:\\Nothing',
				error: { code: 'not_an_install', message: 'E:\\Nothing is not a Palworld installation' }
			},
			context
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('is not a Palworld installation');
		expect(closeModal).not.toHaveBeenCalled();
		expect(screen.getByRole('button', { name: /Choose folder/ }).hasAttribute('disabled')).toBe(
			false
		);
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('cannot be dismissed with Escape or Close while its own pick is in flight', async () => {
		env.desktop = 'true';
		const dismissed = vi.fn();
		const onKeydown = (event: KeyboardEvent) => {
			if (event.key === 'Escape') dismissed();
		};
		window.addEventListener('keydown', onKeydown);
		try {
			const closeModal = vi.fn();
			render(AddTargetModal, { closeModal });
			const choose = screen.getByRole('button', { name: /Choose folder/ });

			await fireEvent.keyDown(choose, { key: 'Escape' });
			expect(dismissed).toHaveBeenCalledTimes(1);

			await fireEvent.click(choose);
			await fireEvent.keyDown(choose, { key: 'Escape' });
			expect(dismissed).toHaveBeenCalledTimes(1);
			expect(screen.getByRole('button', { name: 'Close' }).hasAttribute('disabled')).toBe(true);
			expect(closeModal).not.toHaveBeenCalled();

			await modTargetAddHandler.handle({ root_path: '__select__', canceled: true }, context);
			await tick();
			await fireEvent.keyDown(choose, { key: 'Escape' });
			expect(dismissed).toHaveBeenCalledTimes(2);
			expect(screen.getByRole('button', { name: 'Close' }).hasAttribute('disabled')).toBe(false);
		} finally {
			window.removeEventListener('keydown', onKeydown);
		}
	});

	it("swallows clicks on the dialog's own close controls while its pick is in flight", async () => {
		env.desktop = 'true';
		const dialog = document.createElement('div');
		dialog.setAttribute('role', 'dialog');
		const dialogClose = document.createElement('button');
		const dismissed = vi.fn();
		dialogClose.addEventListener('click', dismissed);
		const content = document.createElement('div');
		dialog.append(dialogClose, content);
		document.body.append(dialog);
		try {
			render(AddTargetModal, { target: content, props: { closeModal: vi.fn() } });

			await fireEvent.click(screen.getByRole('button', { name: /Choose folder/ }));
			await fireEvent.click(dialogClose);
			expect(dismissed).not.toHaveBeenCalled();

			await modTargetAddHandler.handle({ root_path: '__select__', canceled: true }, context);
			await tick();
			await fireEvent.click(dialogClose);
			expect(dismissed).toHaveBeenCalledTimes(1);
		} finally {
			dialog.remove();
		}
	});

	it('ignores an add reply that landed before the modal sent its own request', async () => {
		await modTargetAddHandler.handle({ root_path: '__select__', canceled: true }, context);
		const closeModal = vi.fn();
		render(AddTargetModal, { closeModal });

		await fireEvent.input(screen.getByPlaceholderText(/Path to a Palworld install/), {
			target: { value: 'E:\\Palworld' }
		});
		await fireEvent.click(screen.getByRole('button', { name: 'Add' }));
		await tick();

		expect(closeModal).not.toHaveBeenCalled();
		expect(screen.getByRole('button', { name: /Add/ }).hasAttribute('disabled')).toBe(true);
	});
});
