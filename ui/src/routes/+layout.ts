import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import type { LayoutLoad } from './$types';

export const ssr = false;
export const prerender = true;

function isValidPath(path: string) {
	return [
		'edit',
		'info',
		'file',
		'settings',
		'loading',
		'error',
		'browser',
		'about',
		'upload',
		'presets'
	].includes(path);
}

export const load: LayoutLoad = ({ url }) => {
	const path = url.searchParams.get('path');
	if (path) {
		const decodedPath = decodeURIComponent(path);
		goto(`${resolve('/')}${decodedPath}`);
	}
};
