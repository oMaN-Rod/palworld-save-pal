import type { LiveActorJson, LiveFrameJson } from '$lib/signal/session.svelte';

const STALE_MS = 10_000;

export interface LiveActorsState {
	readonly actors: LiveActorJson[];
	readonly frameAt: number | null;
	readonly stale: boolean;
	readonly followedId: string | null;
	readonly followEpoch: number;
	applyFrame(frame: LiveFrameJson): void;
	clear(): void;
	follow(id: string): void;
	unfollow(): void;
}

class LiveActorsStore implements LiveActorsState {
	#actors = $state.raw<LiveActorJson[]>([]);
	#frameAt = $state<number | null>(null);
	#tick = $state(0);
	#followedId = $state<string | null>(null);
	#followEpoch = $state(0);
	#timer: ReturnType<typeof setInterval> | null = null;
	#now: () => number;

	constructor(now: () => number = Date.now) {
		this.#now = now;
	}

	get actors(): LiveActorJson[] {
		return this.#actors;
	}

	get frameAt(): number | null {
		return this.#frameAt;
	}

	get stale(): boolean {
		void this.#tick;
		return this.#frameAt !== null && this.#now() - this.#frameAt > STALE_MS;
	}

	get followedId(): string | null {
		return this.#followedId;
	}

	get followEpoch(): number {
		return this.#followEpoch;
	}

	applyFrame(frame: LiveFrameJson): void {
		this.#actors = frame.actors;
		this.#frameAt = this.#now();
		this.#ensureTicking();
	}

	clear(): void {
		this.#actors = [];
		this.#frameAt = null;
		this.#stopTicking();
		this.unfollow();
	}

	follow(id: string): void {
		this.#followedId = id;
		this.#followEpoch++;
	}

	unfollow(): void {
		this.#followedId = null;
	}

	#ensureTicking(): void {
		if (this.#timer || typeof setInterval === 'undefined') return;
		this.#timer = setInterval(() => this.#tick++, 1000);
	}

	#stopTicking(): void {
		if (!this.#timer) return;
		clearInterval(this.#timer);
		this.#timer = null;
	}
}

let instance: LiveActorsStore | null = null;

export function getLiveActors(): LiveActorsState {
	if (!instance) instance = new LiveActorsStore();
	return instance;
}

