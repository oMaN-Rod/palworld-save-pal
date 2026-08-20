// Free of three.js and the DOM so its feel is testable without a GL context -- the viewer feeds it frame times and pointer x, and reads `angle`.
const TAU = Math.PI * 2;

export const REVOLUTION_MS = 20000;
export const AUTO_SPIN_RAD_PER_MS = TAU / REVOLUTION_MS;
// A ~300 px drag turns the Pal all the way round, so a modal-width sweep is a full inspection.
export const DRAG_RAD_PER_PX = TAU / 300;
// Past a revolution per second a flick reads as blur, and trackpads report far larger deltas than that.
export const MAX_SPIN_RAD_PER_MS = TAU / 1000;
// Time constant of the ease from a released flick back to the idle rate.
const SETTLE_MS = 700;

function normalize(angle: number): number {
	return ((angle % TAU) + TAU) % TAU;
}

export class PalSpin {
	angle = 0;
	velocity = AUTO_SPIN_RAD_PER_MS;

	#dragging = false;
	#lastX = 0;
	#lastT = 0;

	advance(dtMs: number): number {
		if (this.#dragging || dtMs <= 0) return this.angle;
		// Exponential rather than a fixed step so the return to idle is frame-rate independent.
		this.velocity += (AUTO_SPIN_RAD_PER_MS - this.velocity) * (1 - Math.exp(-dtMs / SETTLE_MS));
		this.angle = normalize(this.angle + this.velocity * dtMs);
		return this.angle;
	}

	pointerDown(x: number, tMs: number): void {
		this.#dragging = true;
		this.#lastX = x;
		this.#lastT = tMs;
		// Otherwise the last flick's leftover velocity is handed back at release, spinning a Pal the user grabbed to hold still.
		this.velocity = 0;
	}

	pointerMove(x: number, tMs: number): void {
		if (!this.#dragging) return;
		const dx = x - this.#lastX;
		const dt = tMs - this.#lastT;
		this.angle = normalize(this.angle + dx * DRAG_RAD_PER_PX);
		if (dt > 0) {
			const v = (dx * DRAG_RAD_PER_PX) / dt;
			this.velocity = Math.max(-MAX_SPIN_RAD_PER_MS, Math.min(MAX_SPIN_RAD_PER_MS, v));
		}
		this.#lastX = x;
		this.#lastT = tMs;
	}

	pointerUp(): void {
		this.#dragging = false;
	}
}
