import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRenderFpsMonitor } from './fpsMonitor';

describe('createRenderFpsMonitor', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('reports painted frames per second over the window', () => {
		const monitor = createRenderFpsMonitor(500);
		const stop = monitor.start();
		// 30 bumps per 500ms window = 60fps.
		for (let i = 0; i < 30; i++) monitor.bump();
		vi.advanceTimersByTime(500);
		const sample = monitor.sample();
		expect(sample.rendered).toBe(true);
		expect(sample.fps).toBeCloseTo(60, 0);
		stop();
	});

	it('idle windows report rendered:false rather than a phantom rate', () => {
		const monitor = createRenderFpsMonitor(500);
		const stop = monitor.start();
		vi.advanceTimersByTime(500);
		expect(monitor.sample()).toEqual({ fps: 0, rendered: false });
		stop();
	});

	it('delivers a sample per window through onSample', () => {
		const monitor = createRenderFpsMonitor(500);
		const seen: boolean[] = [];
		const unsubscribe = monitor.onSample((s) => seen.push(s.rendered));
		const stop = monitor.start();
		monitor.bump();
		vi.advanceTimersByTime(500);
		vi.advanceTimersByTime(500);
		expect(seen).toEqual([true, false]);
		unsubscribe();
		stop();
	});

	it('a stopped monitor stops sampling', () => {
		const monitor = createRenderFpsMonitor(500);
		const stop = monitor.start();
		stop();
		monitor.bump();
		vi.advanceTimersByTime(1_000);
		expect(monitor.sample().rendered).toBe(false);
	});
});
