// @vitest-environment jsdom
import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import './fixtures/animatePolyfill';

import LiveGuildSidebar from '../LiveGuildSidebar.svelte';

describe('LiveGuildSidebar base camp level', () => {
	it('seeds the input from the guild-reported level', async () => {
		render(LiveGuildSidebar, {
			props: { baseCampLevel: 35, activeView: 'members', onSelectView: vi.fn() }
		});

		expect(await screen.findByDisplayValue('35')).toBeTruthy();
	});

	it('re-seeds when the reported level changes underneath an untouched input', async () => {
		const { rerender } = render(LiveGuildSidebar, {
			props: { baseCampLevel: 35, activeView: 'members', onSelectView: vi.fn() }
		});
		await screen.findByDisplayValue('35');

		await rerender({ baseCampLevel: 40, activeView: 'members', onSelectView: vi.fn() });

		expect(await screen.findByDisplayValue('40')).toBeTruthy();
	});

	it('sends the drafted level through onSetBaseCampLevel', async () => {
		const user = userEvent.setup();
		const onSetBaseCampLevel = vi.fn();
		render(LiveGuildSidebar, {
			props: {
				baseCampLevel: 35,
				activeView: 'members',
				onSelectView: vi.fn(),
				onSetBaseCampLevel
			}
		});

		const input = await screen.findByLabelText('Base camp level');
		await user.clear(input);
		await user.type(input, '12');
		await user.click(screen.getByRole('button', { name: 'Set level' }));

		expect(onSetBaseCampLevel).toHaveBeenCalledWith(12);
	});

	it('refuses a level outside 1-50 and shows why instead of calling the callback', async () => {
		const user = userEvent.setup();
		const onSetBaseCampLevel = vi.fn();
		render(LiveGuildSidebar, {
			props: {
				baseCampLevel: 35,
				activeView: 'members',
				onSelectView: vi.fn(),
				onSetBaseCampLevel
			}
		});

		const input = await screen.findByLabelText('Base camp level');
		await user.clear(input);
		await user.type(input, '99');
		await user.click(screen.getByRole('button', { name: 'Set level' }));

		expect(onSetBaseCampLevel).not.toHaveBeenCalled();
		expect(await screen.findByText(/between 1 and 50/)).toBeTruthy();
	});

	it('disables the control when setLevelReason is set', async () => {
		render(LiveGuildSidebar, {
			props: {
				baseCampLevel: 35,
				activeView: 'members',
				onSelectView: vi.fn(),
				setLevelReason: 'World not writable'
			}
		});

		const button = await screen.findByRole('button', { name: 'Set level' });
		expect((button as HTMLButtonElement).disabled).toBe(true);
	});
});

const twoGuilds = [
	{ id: 'g1', name: 'A', adminUid: null, members: null },
	{ id: 'g2', name: 'B', adminUid: null, members: null }
];

describe('LiveGuildSidebar guild picker', () => {
	it('asks for the guild that was picked', async () => {
		const user = userEvent.setup();
		const onSelectGuild = vi.fn();
		render(LiveGuildSidebar, {
			props: {
				guildChoices: twoGuilds,
				selectedGuildId: 'g1',
				onSelectGuild,
				activeView: 'members',
				onSelectView: vi.fn()
			}
		});

		await user.selectOptions(await screen.findByLabelText(/switch guild/i), 'g2');

		expect(onSelectGuild).toHaveBeenCalledWith('g2');
	});
});

describe('LiveGuildSidebar header stats', () => {
	it('reports the base count and member count', async () => {
		render(LiveGuildSidebar, {
			props: { baseCount: 4, memberCount: 7, activeView: 'members', onSelectView: vi.fn() }
		});

		expect(await screen.findByText('4')).toBeTruthy();
		expect(await screen.findByText('7')).toBeTruthy();
	});

	it('shows an em dash rather than a blank cell when the base count is unknown', async () => {
		render(LiveGuildSidebar, {
			props: {
				name: 'DeBugging',
				baseCount: null,
				memberCount: 0,
				activeView: 'members',
				onSelectView: vi.fn()
			}
		});

		expect(await screen.findByText('—')).toBeTruthy();
	});
});

describe('LiveGuildSidebar narrow state', () => {
	it('is a fixed rail that only widens past the measured 700px pane', async () => {
		const { container } = render(LiveGuildSidebar, {
			props: { activeView: 'members', onSelectView: vi.fn() }
		});
		await screen.findByRole('button', { name: /members/i });

		const sidebar = container.querySelector('#live-guild-sidebar') as HTMLElement;
		expect(sidebar).toBeTruthy();
		expect(sidebar.className).toMatch(/(^|\s)w-13(\s|$)/);
		expect(sidebar.className).not.toMatch(/(^|\s)w-full(\s|$)/);
		expect(sidebar.className).toContain('@min-[700px]/guild:w-54');
		expect(sidebar.className).not.toMatch(/@(max-)?sm\/guild/);
	});

	it('offers a settings popover trigger for the header controls the rail hides', async () => {
		render(LiveGuildSidebar, {
			props: { activeView: 'members', onSelectView: vi.fn() }
		});

		const trigger = await screen.findByRole('button', { name: 'Guild settings' });
		expect(trigger).toBeTruthy();
	});

	it('hides the nav labels in the rail and shows them past the same breakpoint', async () => {
		const { container } = render(LiveGuildSidebar, {
			props: {
				counts: { members: '7' },
				activeView: 'members',
				onSelectView: vi.fn()
			}
		});
		await screen.findByRole('navigation');
		const nav = container.querySelector('nav') as HTMLElement;

		const label = within(nav).getByText('Members');
		expect(label.className).toContain('sr-only');
		expect(label.className).toContain('@min-[700px]/guild:not-sr-only');

		const count = within(nav).getByText('7');
		expect(count.className).toContain('sr-only');
		expect(count.className).toContain('@min-[700px]/guild:not-sr-only');
	});
});
