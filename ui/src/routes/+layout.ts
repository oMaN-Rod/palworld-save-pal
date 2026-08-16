import { browser } from '$app/environment';
import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import type { LayoutLoad } from './$types';

export const ssr = false;
export const prerender = true;

export const load: LayoutLoad = ({ url }) => {
	// Runs in Node during prerender for routes that opt into SSR, where there is
	// nothing to navigate.
	if (!browser) return;
	const path = url.searchParams.get('path');
	if (path) {
		const decodedPath = decodeURIComponent(path);
		goto(`${resolve('/')}${decodedPath}`);
	}
};
