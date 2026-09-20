// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest';

import { installViewportStub } from './__tests__/fixtures/viewportStub';

// Imported dynamically so the stub is live before `layout.svelte` builds its `MediaQuery`s.
const { setViewport } = installViewportStub();
const { BREAKPOINTS, layout } = await import('./layout.svelte');
type DeviceClass = import('./layout.svelte').DeviceClass;

describe('layout', () => {
	beforeEach(() => setViewport(1440));

	it('exposes the Tailwind boundaries it derives from', () => {
		expect(BREAKPOINTS.md).toBe(768);
		expect(BREAKPOINTS.xl).toBe(1280);
	});

	it('classifies a phone', () => {
		setViewport(390);
		expect(layout.deviceClass satisfies DeviceClass).toBe('phone');
		expect(layout.phone).toBe(true);
		expect(layout.tablet).toBe(false);
		expect(layout.desktop).toBe(false);
	});

	it('classifies a tablet', () => {
		setViewport(820);
		expect(layout.deviceClass).toBe('tablet');
		expect(layout.tablet).toBe(true);
	});

	it('classifies a desktop', () => {
		setViewport(1440);
		expect(layout.deviceClass).toBe('desktop');
		expect(layout.desktop).toBe(true);
	});

	it('treats the 768 and 1280 boundaries as inclusive lower bounds', () => {
		setViewport(767);
		expect(layout.deviceClass).toBe('phone');
		setViewport(768);
		expect(layout.deviceClass).toBe('tablet');
		setViewport(1279);
		expect(layout.deviceClass).toBe('tablet');
		setViewport(1280);
		expect(layout.deviceClass).toBe('desktop');
	});

	it('reports pointer type independently of width', () => {
		setViewport(1440, { coarse: true });
		expect(layout.desktop).toBe(true);
		expect(layout.coarse).toBe(true);
	});
});
