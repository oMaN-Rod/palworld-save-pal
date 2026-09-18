import * as m from '$i18n/messages';
import type { ModError } from '$types';

/** The server's own message is the fallback; it may be Nexus's text, so never render it as HTML. */
export function nexusErrorText(error: ModError): string {
	switch (error.code) {
		case 'key_required':
			return m.mods_nexus_error_key_required();
		case 'invalid_key':
			return m.mods_nexus_error_invalid_key();
		case 'keyring_unavailable':
			return m.mods_nexus_error_keyring();
		case 'desktop_only':
			return m.mods_nexus_error_desktop_only();
		case 'network':
			return m.mods_nexus_error_network();
		case 'rate_limited': {
			const reset = typeof error.reset === 'string' ? error.reset : null;
			return reset
				? m.mods_nexus_error_rate_limited({ reset })
				: m.mods_nexus_error_rate_limited_soon();
		}
		case 'nexus_error':
			return m.mods_nexus_error_nexus({ status: String(error.status ?? '') });
		case 'no_executable':
			return m.mods_nexus_error_no_executable();
		case 'register_failed':
			return m.mods_nexus_error_register_failed();
		case 'unsupported_platform':
			return m.mods_nexus_error_unsupported_platform();
		default:
			return error.message;
	}
}

const UNITS: { seconds: number; unit: Intl.RelativeTimeFormatUnit }[] = [
	{ seconds: 86400, unit: 'day' },
	{ seconds: 3600, unit: 'hour' },
	{ seconds: 60, unit: 'minute' },
	{ seconds: 1, unit: 'second' }
];

const ABSOLUTE_AFTER = 30 * 86400;

/**
 * "3 days ago" while a date is still fresh, a plain date once it is older than a month.
 * Null when there is no usable date.
 */
export function nexusDateText(iso: string | null, now = Date.now()): string | null {
	if (!iso) return null;
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return null;
	const elapsed = Math.max(0, (now - date.getTime()) / 1000);
	if (elapsed >= ABSOLUTE_AFTER) return date.toLocaleDateString(undefined, { dateStyle: 'medium' });
	const { seconds, unit } =
		UNITS.find((entry) => elapsed >= entry.seconds) ?? UNITS[UNITS.length - 1];
	const relative = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });
	return relative.format(-Math.round(elapsed / seconds), unit);
}
