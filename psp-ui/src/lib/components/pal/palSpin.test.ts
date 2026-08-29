import { describe, expect, it } from 'vitest';
import {
	AUTO_SPIN_RAD_PER_MS,
	DRAG_RAD_PER_PX,
	MAX_SPIN_RAD_PER_MS,
	PalSpin,
	REVOLUTION_MS
} from './palSpin';

const TAU = Math.PI * 2;

describe('PalSpin', () => {
	it('turns a quarter revolution in a quarter of the revolution period', () => {
		const spin = new PalSpin();

		spin.advance(REVOLUTION_MS / 4);

		expect(spin.angle).toBeCloseTo(Math.PI / 2, 5);
	});

	it('keeps the angle within one revolution however long it spins', () => {
		const spin = new PalSpin();

		for (let i = 0; i < 500; i++) spin.advance(1000);

		expect(spin.angle).toBeGreaterThanOrEqual(0);
		expect(spin.advance(0)).toBeLessThan(TAU);
	});

	it('ignores a frame that took no time', () => {
		const spin = new PalSpin();
		spin.advance(500);
		const before = spin.angle;

		spin.advance(0);
		spin.advance(-16);

		expect(spin.angle).toBe(before);
	});

	it('holds still while the pointer is down', () => {
		const spin = new PalSpin();
		spin.pointerDown(100, 0);

		spin.advance(5000);

		expect(spin.angle).toBe(0);
	});

	it('turns with the pointer while dragging', () => {
		const spin = new PalSpin();
		spin.pointerDown(100, 0);

		spin.pointerMove(160, 16);

		expect(spin.angle).toBeCloseTo(60 * DRAG_RAD_PER_PX, 6);
	});

	it('turns the other way when the pointer goes the other way', () => {
		const spin = new PalSpin();
		spin.pointerDown(100, 0);

		spin.pointerMove(40, 16);

		expect(spin.angle).toBeCloseTo(TAU - 60 * DRAG_RAD_PER_PX, 6);
	});

	it('ignores pointer movement that never started with a press', () => {
		const spin = new PalSpin();

		spin.pointerMove(400, 16);

		expect(spin.angle).toBe(0);
	});

	it('resumes spinning on its own once the pointer is released', () => {
		const spin = new PalSpin();
		spin.pointerDown(100, 0);
		spin.pointerMove(110, 16);
		spin.pointerUp();
		const released = spin.angle;

		spin.advance(1000);

		expect(spin.angle).toBeGreaterThan(released);
	});

	it('carries a flick past the release before easing back to the idle rate', () => {
		const spin = new PalSpin();
		spin.pointerDown(0, 0);
		spin.pointerMove(120, 16);
		spin.pointerUp();

		const before = spin.angle;
		spin.advance(16);

		expect(spin.angle - before).toBeGreaterThan(AUTO_SPIN_RAD_PER_MS * 16 * 3);
	});

	it('settles back to the idle rate a few seconds after a flick', () => {
		const spin = new PalSpin();
		spin.pointerDown(0, 0);
		spin.pointerMove(120, 16);
		spin.pointerUp();

		for (let i = 0; i < 500; i++) spin.advance(16);
		const idleStep = AUTO_SPIN_RAD_PER_MS * 16;
		const before = spin.angle;
		spin.advance(16);

		expect((spin.angle - before) / idleStep).toBeCloseTo(1, 2);
	});

	it('caps how fast an extreme flick can spin it', () => {
		const spin = new PalSpin();
		spin.pointerDown(0, 0);

		spin.pointerMove(9000, 1);

		expect(spin.velocity).toBeLessThanOrEqual(MAX_SPIN_RAD_PER_MS);
	});

	it('drops the leftover flick speed when the pointer comes down again', () => {
		const spin = new PalSpin();
		spin.pointerDown(0, 0);
		spin.pointerMove(120, 16);
		spin.pointerUp();

		spin.pointerDown(0, 100);

		expect(spin.velocity).toBe(0);
	});
});
