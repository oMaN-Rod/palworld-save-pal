import * as m from '$i18n/messages';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';

import Signal from './Signal.svelte';

describe('Signal landing section', () => {
	const body = render(Signal).body;

	it('explains the feature and links to the Remote Access guide', () => {
		expect(body).toContain(m.landing_signal_heading());
		expect(body).toContain(m.landing_signal_body());
		expect(body).toContain('href="/docs/guides/remote-access"');
	});

	it('lists live map, save editing, server management and the minimap', () => {
		expect(body).toContain(m.landing_signal_feature_live_title());
		expect(body).toContain(m.landing_signal_feature_edit_title());
		expect(body).toContain(m.landing_signal_feature_servers_title());
		expect(body).toContain(m.landing_signal_feature_minimap_title());
	});

	it('walks through pairing in order', () => {
		const arm = body.indexOf(m.landing_signal_step_arm_title());
		const pair = body.indexOf(m.landing_signal_step_pair_title());
		const watch = body.indexOf(m.landing_signal_step_watch_title());
		expect(arm).toBeGreaterThan(-1);
		expect(pair).toBeGreaterThan(arm);
		expect(watch).toBeGreaterThan(pair);
	});

	it('states the privacy posture', () => {
		expect(body).toContain(m.landing_signal_point_p2p_title());
		expect(body).toContain(m.landing_signal_point_broker_title());
		expect(body).toContain(m.landing_signal_point_control_title());
	});
});
