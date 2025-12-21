import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import type { LayoutLoad } from './$types';

export const ssr = false;
export const prerender = true;

export const load: LayoutLoad = ({ url }) => {
	const path = url.searchParams.get('path');
	if (path) {
		const decodedPath = decodeURIComponent(path);
		goto(`${resolve('/')}${decodedPath}`);
	}
};
