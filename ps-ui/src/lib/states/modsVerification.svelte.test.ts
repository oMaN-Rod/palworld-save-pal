import { beforeEach, describe, expect, it, vi } from 'vitest';

const { send, sendAndWait } = vi.hoisted(() => ({ send: vi.fn(), sendAndWait: vi.fn() }));
vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: string, data?: unknown) => send(type, data),
	sendAndWait
}));

import { ModsState } from './modsState.svelte';

describe('verification subscription', () => {
	let state: ModsState;
	beforeEach(() => {
		send.mockReset();
		state = new ModsState();
	});

	it('subscribes once per connection and again after a drop', () => {
		state.connectionChanged(true);
		state.subscribeVerification();
		state.subscribeVerification();
		expect(send.mock.calls).toEqual([['mod_verification_subscribe', {}]]);
		state.connectionChanged(false);
		state.connectionChanged(true);
		state.subscribeVerification();
		expect(send.mock.calls.length).toBe(2);
	});

	it('finds a mod status by id', () => {
		state.verification = {
			t1: {
				target_id: 't1',
				instance_id: 'auto:1',
				live: true,
				checked_at: '',
				build_info: null,
				resolution: { complete: true, missing: [] },
				status: [{ mod_id: 'm1', name: 'M1', kind: 'ue4ss', status: 'verified' }]
			}
		};
		expect(state.verificationFor('t1', 'm1')?.status).toBe('verified');
		expect(state.verificationFor('t1', 'nope')).toBeUndefined();
	});
});
