const STORAGE_KEY = 'psp-signal-device';

export interface StoredDesktop {
	meetRoom: string;
	deviceId: string;
	deviceSecret: string;
	desktopName: string;
	pairedAtMs: number;
}

function isStoredDesktop(value: unknown): value is StoredDesktop {
	if (!value || typeof value !== 'object') return false;
	const v = value as Record<string, unknown>;
	return (
		typeof v.meetRoom === 'string' &&
		typeof v.deviceId === 'string' &&
		typeof v.deviceSecret === 'string' &&
		typeof v.desktopName === 'string' &&
		typeof v.pairedAtMs === 'number'
	);
}

export function loadStoredDesktop(): StoredDesktop | null {
	let raw: string | null;
	try {
		raw = localStorage.getItem(STORAGE_KEY);
	} catch {
		return null;
	}
	if (!raw) return null;

	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		clearStoredDesktop();
		return null;
	}

	if (!isStoredDesktop(parsed)) {
		clearStoredDesktop();
		return null;
	}
	return parsed;
}

export function saveStoredDesktop(d: StoredDesktop): void {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(d));
	} catch {
	}
}

export function clearStoredDesktop(): void {
	try {
		localStorage.removeItem(STORAGE_KEY);
	} catch {
	}
}

const BROWSERS: Array<[RegExp, string]> = [
	[/Edg\//, 'Edge'],
	[/OPR\//, 'Opera'],
	[/Firefox\//, 'Firefox'],
	[/Chrome\//, 'Chrome'],
	[/Version\/.*Safari\//, 'Safari']
];

const OSES: Array<[RegExp, string]> = [
	[/iPhone/, 'iPhone'],
	[/iPad/, 'iPad'],
	[/Android/, 'Android'],
	[/Mac OS X/, 'macOS'],
	[/Windows/, 'Windows'],
	[/Linux/, 'Linux']
];

function match(ua: string, table: Array<[RegExp, string]>): string | null {
	for (const [re, name] of table) {
		if (re.test(ua)) return name;
	}
	return null;
}

const FALLBACK_LABEL = 'This browser';

export function deviceLabel(): string {
	try {
		const ua = navigator.userAgent;
		const browser = match(ua, BROWSERS);
		const os = match(ua, OSES);
		if (!browser || !os) return FALLBACK_LABEL;
		return `${browser} on ${os}`;
	} catch {
		return FALLBACK_LABEL;
	}
}
