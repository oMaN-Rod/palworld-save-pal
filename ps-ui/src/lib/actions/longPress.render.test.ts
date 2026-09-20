// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { longPress } from './longPress';

function pointerEvent(type: string, x: number, y: number): Event {
	const event = new Event(type, { bubbles: true, cancelable: true });
	Object.defineProperty(event, 'clientX', { value: x });
	Object.defineProperty(event, 'clientY', { value: y });
	Object.defineProperty(event, 'pointerId', { value: 1 });
	return event;
}

describe('longPress', () => {
	let node: HTMLElement;

	beforeEach(() => {
		vi.useFakeTimers();
		node = document.createElement('button');
		// jsdom does not implement pointer capture.
		node.setPointerCapture = () => {};
		node.releasePointerCapture = () => {};
		document.body.appendChild(node);
	});

	afterEach(() => {
		vi.useRealTimers();
		node.remove();
	});

	it('fires after the delay when the pointer stays put', () => {
		const onLongPress = vi.fn();
		longPress(node, { onLongPress });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		vi.advanceTimersByTime(500);

		expect(onLongPress).toHaveBeenCalledTimes(1);
	});

	it('does not fire before the delay', () => {
		const onLongPress = vi.fn();
		longPress(node, { onLongPress });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		vi.advanceTimersByTime(499);

		expect(onLongPress).not.toHaveBeenCalled();
	});

	it('cancels when the pointer moves past the tolerance', () => {
		const onLongPress = vi.fn();
		longPress(node, { onLongPress });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		node.dispatchEvent(pointerEvent('pointermove', 10, 40));
		vi.advanceTimersByTime(500);

		expect(onLongPress).not.toHaveBeenCalled();
	});

	it('tolerates a small jitter', () => {
		const onLongPress = vi.fn();
		longPress(node, { onLongPress });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		node.dispatchEvent(pointerEvent('pointermove', 14, 13));
		vi.advanceTimersByTime(500);

		expect(onLongPress).toHaveBeenCalledTimes(1);
	});

	it('cancels when the pointer lifts early', () => {
		const onLongPress = vi.fn();
		longPress(node, { onLongPress });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		vi.advanceTimersByTime(200);
		node.dispatchEvent(pointerEvent('pointerup', 10, 10));
		vi.advanceTimersByTime(500);

		expect(onLongPress).not.toHaveBeenCalled();
	});

	it('suppresses the click that follows a fired long-press', () => {
		const onLongPress = vi.fn();
		const onClick = vi.fn();
		node.addEventListener('click', onClick);
		longPress(node, { onLongPress });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		vi.advanceTimersByTime(500);
		node.dispatchEvent(pointerEvent('pointerup', 10, 10));
		node.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));

		expect(onClick).not.toHaveBeenCalled();
	});

	it('lets a normal click through when no long-press fired', () => {
		const onClick = vi.fn();
		node.addEventListener('click', onClick);
		longPress(node, { onLongPress: vi.fn() });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		vi.advanceTimersByTime(100);
		node.dispatchEvent(pointerEvent('pointerup', 10, 10));
		node.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));

		expect(onClick).toHaveBeenCalledTimes(1);
	});

	it('does nothing while disabled', () => {
		const onLongPress = vi.fn();
		longPress(node, { onLongPress, enabled: false });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		vi.advanceTimersByTime(500);

		expect(onLongPress).not.toHaveBeenCalled();
	});

	it('stops listening once destroyed', () => {
		const onLongPress = vi.fn();
		const handle = longPress(node, { onLongPress });

		handle.destroy();
		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		vi.advanceTimersByTime(500);

		expect(onLongPress).not.toHaveBeenCalled();
	});

	it('does not leak click suppression to the next click', () => {
		const onClick = vi.fn();
		node.addEventListener('click', onClick);
		longPress(node, { onLongPress: vi.fn() });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		vi.advanceTimersByTime(500);
		node.dispatchEvent(pointerEvent('pointerup', 10, 10));
		node.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));

		expect(onClick).toHaveBeenCalledTimes(0);

		node.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));

		expect(onClick).toHaveBeenCalledTimes(1);
	});

	it('properly removes the capture-phase click listener on destroy', () => {
		const onClick = vi.fn();
		node.addEventListener('click', onClick);
		const handle = longPress(node, { onLongPress: vi.fn() });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		vi.advanceTimersByTime(500);
		handle.destroy();

		node.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));

		expect(onClick).toHaveBeenCalledTimes(1);
	});

	it('respects enabled flag when timer fires', () => {
		const onLongPress = vi.fn();
		const handle = longPress(node, { onLongPress, enabled: true });

		node.dispatchEvent(pointerEvent('pointerdown', 10, 10));
		handle.update({ onLongPress, enabled: false });
		vi.advanceTimersByTime(500);

		expect(onLongPress).not.toHaveBeenCalled();
	});
});
