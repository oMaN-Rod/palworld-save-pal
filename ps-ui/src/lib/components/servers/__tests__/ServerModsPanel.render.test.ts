// @vitest-environment jsdom
import type { Server } from '$types';
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { received, holder, send } = vi.hoisted(() => ({
	received: [] as Record<string, unknown>[],
	holder: { mods: undefined as unknown },
	send: vi.fn()
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn()
}));

vi.mock('$components/mods', () => ({
	TargetPanel: (_anchor: unknown, props: Record<string, unknown>) => {
		received.push(props);
	}
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.mods = new ModsState();
	return { getModsState: () => holder.mods };
});

import { modTarget } from '$lib/components/mods/__tests__/fixtures';
import type { ModsState } from '$lib/states/modsState.svelte';
import ServerModsPanel from '../ServerModsPanel.svelte';
import ServerModsPanelHarness from './ServerModsPanelHarness.svelte';

const mods = () => holder.mods as ModsState;

beforeEach(() => {
	received.length = 0;
	send.mockReset();
	mods().reset();
});

describe('ServerModsPanel', () => {
	it('mounts the shared mods panel on the server target', () => {
		render(ServerModsPanel, { server: { id: 7 } as Server });

		expect(received).toHaveLength(1);
		expect(received[0].targetId).toBe('server-7');
		expect(received[0].tab).toBe('mods');
	});

	it('binds the mods tab both ways', async () => {
		render(ServerModsPanelHarness);
		const props = received[0];
		expect(props.tab).toBe('scan');

		props.tab = 'backups';
		await tick();

		expect(screen.getByTestId('tab').textContent).toBe('backups');
	});

	it('reloads the targets and the library after a reconnect, and not on mount', async () => {
		mods().targets = [modTarget({ id: 'server-7', kind: 'server', server_id: 7 })];
		mods().connectionChanged(true);
		render(ServerModsPanel, { server: { id: 7 } as Server });
		await tick();
		expect(send).not.toHaveBeenCalled();

		mods().connectionChanged(false);
		mods().connectionChanged(true);
		await tick();

		expect(send.mock.calls).toEqual([
			['mod_target_list', undefined],
			['mod_list', undefined]
		]);
	});

	it('leaves reloading to the mods panel while the server has no target', async () => {
		mods().connectionChanged(true);
		render(ServerModsPanel, { server: { id: 7 } as Server });

		mods().connectionChanged(false);
		mods().connectionChanged(true);
		await tick();

		expect(send).not.toHaveBeenCalled();
	});
});
