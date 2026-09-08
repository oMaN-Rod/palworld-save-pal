// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import './fixtures/animatePolyfill';

import type { GameGuildsJson, GameGuildJson } from '$states/gameState.svelte';
import LiveGuildPane from '../LiveGuildPane.svelte';

function guild(overrides: Partial<NonNullable<GameGuildJson['guild']>> = {}): GameGuildJson {
	return {
		guild: {
			id: '1d40d91a-4db2-6fae-2dd0-b79c12486e2d',
			name: 'DeBugging',
			groupName: '00000000000000000000000000000001',
			baseCampLevel: 35,
			baseCampIds: ['b1', 'b2', 'b3', 'b4'],
			baseCampPointIds: ['p1', 'p2', 'p3', 'p4'],
			memberUids: ['00000000-0000-0000-0000-000000000001'],
			adminUid: '00000000-0000-0000-0000-000000000001',
			roleOptions: ['GuildMaster', 'SubMaster', 'Member', 'Guest'],
			...overrides,
			bases: overrides.bases ?? null,
			members: overrides.members ?? null,
			lab: overrides.lab ?? null
		},
		status: 'ok'
	};
}

describe('LiveGuildPane', () => {
	it('shows the guild by its own name', async () => {
		render(LiveGuildPane, { guild: guild() });

		expect(await screen.findByText('DeBugging')).toBeTruthy();
	});

	it('never falls back to the group name', () => {
		render(LiveGuildPane, { guild: guild({ name: null }) });

		expect(screen.queryByText('00000000000000000000000000000001')).toBeNull();
	});

	it('says so when the player belongs to no guild', async () => {
		render(LiveGuildPane, { guild: { guild: null, status: 'ok' } });

		expect(await screen.findByText('No Guild found')).toBeTruthy();
	});

	it('surfaces a refusal instead of an empty pane', async () => {
		render(LiveGuildPane, {
			guild: null,
			error: { error: 'world not loaded', code: 'capability_unavailable' }
		});

		expect(await screen.findByText(/world not loaded/)).toBeTruthy();
	});

	it('opens on the members view', async () => {
		render(LiveGuildPane, { guild: guild() });

		expect(await screen.findByRole('button', { name: /members/i })).toBeTruthy();
		expect(screen.getByTestId('live-guild-view-members')).toBeTruthy();
	});

	it('switches views when a sidebar entry is clicked', async () => {
		render(LiveGuildPane, { guild: guild() });

		await userEvent.click(await screen.findByRole('button', { name: /base pals/i }));

		expect(screen.getByTestId('live-guild-view-pals')).toBeTruthy();
		expect(screen.queryByTestId('live-guild-view-members')).toBeNull();
	});

	it('offers the guild picker only when the world holds more than one guild', async () => {
		const one: GameGuildsJson = { guilds: [{ id: 'g1', name: 'A', adminUid: null, members: null }], status: 'ok' };
		const { unmount } = render(LiveGuildPane, { guild: guild(), guilds: one });
		expect(screen.queryByLabelText(/switch guild/i)).toBeNull();
		unmount();

		render(LiveGuildPane, {
			guild: guild(),
			guilds: {
				guilds: [
					{ id: 'g1', name: 'A', adminUid: null, members: null },
					{ id: 'g2', name: 'B', adminUid: null, members: null }
				],
				status: 'ok'
			}
		});

		expect(await screen.findByLabelText(/switch guild/i)).toBeTruthy();
	});

	it('selects the loaded guild in the picker before the player has picked one', async () => {
		render(LiveGuildPane, {
			guild: guild(),
			guilds: {
				guilds: [
					{ id: 'decoy-guild', name: 'Decoy', adminUid: null, members: null },
					{ id: '1d40d91a-4db2-6fae-2dd0-b79c12486e2d', name: 'DeBugging', adminUid: null, members: null }
				],
				status: 'ok'
			}
		});

		const select = (await screen.findByLabelText(/switch guild/i)) as HTMLSelectElement;
		expect(select.value).toBe('1d40d91a-4db2-6fae-2dd0-b79c12486e2d');
	});
});
