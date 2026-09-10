// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import LiveGuildMembers from '../LiveGuildMembers.svelte';

const members = [
	{ uid: 'u1', name: 'Aurora', role: 'GuildMaster', status: 'Online', lastOnlineTicks: null },
	{ uid: 'u2', name: 'Fenrir', role: 'Member', status: 'Offline', lastOnlineTicks: null }
];

describe('LiveGuildMembers', () => {
	it('names each member and marks the leader', async () => {
		render(LiveGuildMembers, { members, adminUid: 'u1', roleOptions: ['GuildMaster', 'Member'] });

		expect(await screen.findByText('Aurora')).toBeTruthy();
		expect(screen.getByText('Fenrir')).toBeTruthy();
		expect(screen.getAllByText(/guild leader/i)).toHaveLength(1);
	});

	it('sends the chosen role for the member it belongs to', async () => {
		const onSetMemberRole = vi.fn();
		render(LiveGuildMembers, {
			members,
			adminUid: 'u1',
			roleOptions: ['GuildMaster', 'Member'],
			onSetMemberRole
		});

		await userEvent.selectOptions(await screen.findByLabelText(/role for fenrir/i), 'GuildMaster');

		expect(onSetMemberRole).toHaveBeenCalledWith('u2', 'GuildMaster');
	});

	it('disables the role selects while a write is in flight', async () => {
		render(LiveGuildMembers, {
			members,
			adminUid: 'u1',
			roleOptions: ['GuildMaster', 'Member'],
			roleBusy: true
		});

		expect((await screen.findByLabelText(/role for fenrir/i)).hasAttribute('disabled')).toBe(true);
	});

	it('falls back to bare uids with an explanation', async () => {
		render(LiveGuildMembers, { members: [], memberUids: ['00000000-0000-0000-0000-000000000001'] });

		expect(await screen.findByText('00000000-0000-0000-0000-000000000001')).toBeTruthy();
		expect(screen.getByText(/reports member ids only/i)).toBeTruthy();
	});

	it('shows a dash rather than a year-1 date for an unusable timestamp', async () => {
		render(LiveGuildMembers, {
			members: [{ uid: 'u1', name: 'Aurora', role: 'Member', status: 'Offline', lastOnlineTicks: 1_518_203_890_000 }],
			roleOptions: ['Member']
		});

		expect(await screen.findByText('Aurora')).toBeTruthy();
		expect(screen.queryByText(/0001/)).toBeNull();
	});

	it('does not crown an unidentified member when the admin is also unidentified', async () => {
		render(LiveGuildMembers, {
			members: [
				{ uid: null, name: 'Ghost', role: 'Member', status: 'Offline', lastOnlineTicks: null },
				...members
			],
			adminUid: null,
			roleOptions: ['GuildMaster', 'Member']
		});

		expect(await screen.findByText('Ghost')).toBeTruthy();
		expect(screen.queryByText(/guild leader/i)).toBeNull();
	});
});
