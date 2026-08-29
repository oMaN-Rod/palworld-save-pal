/**
 * Signal — the live world feed module.
 *
 * Wire shapes mirror `psp-signal`'s manager snapshot (`signal_status_update`
 * payloads) and the actor model (`/v1/live` frames).
 */

export type SignalSourceKind = 'rest' | 'restgamedata' | 'gamedata' | 'fake';

export type SignalFeedState =
	| 'idle'
	| 'waiting'
	| 'auth'
	| 'down'
	| 'stale'
	| 'players'
	| 'world'
	| 'feeding';

export interface SignalActor {
	id: string;
	kind: string;
	x: number;
	y: number;
	alt: number;
	name?: string;
	level?: number;
	stage?: string;
	hp?: number;
	maxHp?: number;
	active?: boolean;
	cls?: string;
	yaw?: number;
	owner?: string;
	tribe?: string;
	guildName?: string;
}

export interface SignalFrame {
	ok: boolean;
	source: string;
	age: number;
	stale: boolean;
	unit: string;
	actors: SignalActor[];
}

export interface SignalStatus {
	running: boolean;
	api: {
		url: string;
		bind: string;
		port: number;
		lanIp: string | null;
		loopbackOnly: boolean;
	};
	access: {
		token: string;
	};
	source: {
		kind: SignalSourceKind | null;
		url: string | null;
		locked: boolean;
		passwordSet: boolean;
	};
	feed: {
		state: SignalFeedState;
		error: string | null;
		age: number | null;
		actors: number;
		stale: boolean;
	};
	frame: SignalFrame;
	config: {
		enabled: boolean;
		bind: string;
		port: number;
		intervalMs: number;
		allowedOrigins: string[];
	};
}

export interface SignalGameDataCandidate {
	path: string;
	exists: boolean;
	origin: 'proton' | 'local' | 'palserver';
}

export interface SignalSourceSelection {
	type: 'rest' | 'gamedata' | 'fake';
	url?: string;
	password?: string;
	path?: string;
}

export interface SignalConfigUpdate {
	enabled?: boolean;
	bind?: string;
	port?: number;
	intervalMs?: number;
	allowedOrigins?: string[];
	applyNow?: boolean;
}
