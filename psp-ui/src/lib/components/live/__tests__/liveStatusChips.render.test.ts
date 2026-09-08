// @vitest-environment jsdom
import type { GameStatusJson } from '$states/gameState.svelte';
import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import LiveStatusChips from '../LiveStatusChips.svelte';

function status(overrides: Partial<GameStatusJson> = {}): GameStatusJson {
	return {
		authoritative: true,
		modVersion: '1.0.0',
		mode: 'coop_host',
		protocolVersion: 1,
		queueDepth: 0,
		worldLoaded: true,
		...overrides
	} as GameStatusJson;
}

describe('LiveStatusChips', () => {
	it('names the mod version, mode, authority and world', () => {
		const { getByText } = render(LiveStatusChips, { props: { status: status() } });

		expect(getByText('v1.0.0')).toBeTruthy();
		expect(getByText('Solo/Host')).toBeTruthy();
		expect(getByText('Host')).toBeTruthy();
		expect(getByText('World loaded')).toBeTruthy();
	});

	it('marks an observer connection apart from a host one', () => {
		const { getByText } = render(LiveStatusChips, {
			props: { status: status({ authoritative: false }) }
		});

		const chip = getByText('Observer');
		expect(chip.className).toContain('yellow');
	});

	it('marks a world that is not loaded yet', () => {
		const { getByText } = render(LiveStatusChips, {
			props: { status: status({ worldLoaded: false }) }
		});

		const chip = getByText('World not loaded');
		expect(chip.className).toContain('yellow');
	});

	it('spells a dedicated server apart from a solo host', () => {
		const { getByText } = render(LiveStatusChips, {
			props: { status: status({ mode: 'dedicated' }) }
		});

		expect(getByText('Dedicated')).toBeTruthy();
	});
});
