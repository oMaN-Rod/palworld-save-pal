// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import BottomSheetHarness from './BottomSheetHarness.svelte';
import GatedSheetHarness from './GatedSheetHarness.svelte';

const spyOnAnimate = () => vi.spyOn(Element.prototype, 'animate');

let animate: ReturnType<typeof spyOnAnimate>;

beforeEach(() => {
	animate = spyOnAnimate();
});

afterEach(() => {
	animate.mockRestore();
});

describe('BottomSheet transitions', () => {
	it('slides in when its own open flag flips', async () => {
		const { rerender } = render(BottomSheetHarness, { open: false, onClose: vi.fn() });
		animate.mockClear();

		await rerender({ open: true });
		await tick();

		expect(screen.getByRole('dialog')).not.toBeNull();
		expect(animate).toHaveBeenCalled();
	});

	it('slides in when a parent block brings it into being', async () => {
		const { rerender } = render(GatedSheetHarness, { active: false, onClose: vi.fn() });
		animate.mockClear();

		await rerender({ active: true });
		await tick();

		expect(screen.getByRole('dialog')).not.toBeNull();
		expect(animate).toHaveBeenCalled();
	});

	it('slides out when a parent block takes it away', async () => {
		const { rerender } = render(GatedSheetHarness, { active: true, onClose: vi.fn() });
		await tick();
		animate.mockClear();

		await rerender({ active: false });
		await tick();

		expect(animate).toHaveBeenCalled();
	});

	it('slides from its own bottom edge whatever height it snaps to', async () => {
		const { rerender } = render(GatedSheetHarness, { active: false, onClose: vi.fn() });
		animate.mockClear();

		await rerender({ active: true });
		await tick();

		const frames = animate.mock.calls.flatMap((call) => call[0] as Keyframe[]);
		const css = frames.map((frame) => JSON.stringify(frame)).join(' ');
		expect(css).toContain('100%');
		expect(css).not.toContain('400px');
	});

	it('eases between its snap points', async () => {
		render(BottomSheetHarness, { open: true, onClose: vi.fn() });
		await tick();

		expect(screen.getByRole('dialog').className).toContain('transition-[height]');
	});
});
