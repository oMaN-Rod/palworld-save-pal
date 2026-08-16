import { deLocalizeUrl } from '$i18n/runtime';
import type { Reroute } from '@sveltejs/kit';

// The locale prefix is a public URL concern only. Stripping it here lets
// SvelteKit keep resolving against the existing unprefixed route tree.
export const reroute: Reroute = ({ url }) => deLocalizeUrl(url).pathname;
