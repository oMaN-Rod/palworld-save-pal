import { goto } from '$app/navigation';
import { base } from '$app/paths';
import type { LayoutLoad } from './$types';

export const ssr = false;
export const prerender = true;

function isValidPath(path: string) {
	return ['edit', 'info', 'file', 'settings', 'loading', 'error', 'browser', 'about'].includes(
		path
	);
}

export const load: LayoutLoad = ({ url }) => {
	const path = url.searchParams.get('path');
	if (path) {
		const decodedPath = decodeURIComponent(path);
		if (isValidPath(decodedPath.replace(/^\//, ''))) {
			goto(`${base}${decodedPath}`);
		} else {
			console.warn(`Invalid path: ${decodedPath}, redirecting to base path ${base}`);
			goto(base);
		}
	}
};
