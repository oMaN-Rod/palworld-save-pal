import { render } from 'svelte/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import * as m from '$i18n/messages';

const remoteModeMocks = vi.hoisted(() => ({ active: false }));
vi.mock('$lib/signal/remoteMode.svelte', () => ({
	getRemoteMode: () => ({
		get active() {
			return remoteModeMocks.active;
		}
	})
}));

import Hero from './Hero.svelte';

const html = (props: Record<string, unknown> = {}) =>
	render(Hero, {
		props: {
			onLoad: () => {},
			onResume: () => {},
			resumeName: 'World',
			...props
		}
	}).body;

beforeEach(() => {
	remoteModeMocks.active = false;
});

describe('Hero remote-mode gating', () => {
	it('renders the dropzone and resume button outside remote mode', () => {
		const body = html();
		expect(body).toContain(m.upload_drop_heading());
		expect(body).toContain(m.landing_hero_resume());
		expect(body).not.toContain(m.signal_remote_mode_hint());
	});

	it('hides the dropzone and resume button, and shows the hint, in remote mode', () => {
		remoteModeMocks.active = true;
		const body = html();
		expect(body).not.toContain(m.upload_drop_heading());
		expect(body).not.toContain(m.landing_hero_resume());
		expect(body).toContain(m.signal_remote_mode_hint());
	});
});
