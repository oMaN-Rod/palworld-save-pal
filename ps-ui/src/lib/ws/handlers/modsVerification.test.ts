import { beforeEach, describe, expect, it, vi } from 'vitest';

const { send, toast, holder } = vi.hoisted(() => ({
	send: vi.fn(),
	toast: { add: vi.fn() },
	holder: { state: undefined as unknown }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn()
}));

vi.mock('$i18n/messages', () => ({
	mods_panel_title: () => 'Mods'
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
import type { TargetVerification } from '$types';
import { MessageType } from '$types';
import { modVerificationGetHandler, modVerificationHandler } from './modsHandler';

const context = { goto: vi.fn() };
let state: ModsState;

function verification(overrides: Partial<TargetVerification> = {}): TargetVerification {
	return {
		target_id: 't1',
		instance_id: 'auto:1',
		live: true,
		checked_at: '',
		build_info: null,
		resolution: { complete: true, missing: [] },
		status: [],
		...overrides
	};
}

beforeEach(async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	state = holder.state as ModsState;
	send.mockReset();
	toast.add.mockReset();
});

describe('mod_verification', () => {
	it('stores a push by target_id', async () => {
		await modVerificationHandler.handle(verification(), context);
		expect(state.verification.t1.live).toBe(true);
	});
});

describe('mod_verification_get', () => {
	it('stores the verification for the target', async () => {
		await modVerificationGetHandler.handle(
			{ target_id: 't1', verification: verification({ live: false }) },
			context
		);
		expect(state.verification.t1.live).toBe(false);
	});

	it('deletes the entry when verification is null', async () => {
		state.verification = { t1: verification() };
		await modVerificationGetHandler.handle({ target_id: 't1', verification: null }, context);
		expect(state.verification.t1).toBeUndefined();
	});

	it('records a refusal on the target without toasting', async () => {
		await modVerificationGetHandler.handle(
			{ target_id: 't1', error: { code: 'target_not_found', message: 'gone' } },
			context
		);
		expect(state.lastErrorFor(MessageType.MOD_VERIFICATION_GET, 't1')?.code).toBe(
			'target_not_found'
		);
		expect(toast.add).not.toHaveBeenCalled();
	});
});
