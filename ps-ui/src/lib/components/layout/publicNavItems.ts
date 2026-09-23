import * as m from '$i18n/messages';

export type PublicNavItem = {
	id: string;
	href: string;
	/** Iconify icon name, e.g. `tabler:map`. */
	icon: string;
	label: () => string;
	hideOnMobile?: boolean;
};

export const publicNavItems: PublicNavItem[] = [
	{ id: 'map', href: '/map', icon: 'tabler:map', label: () => m.map() },
	{ id: 'wiki', href: '/wiki', icon: 'tabler:book', label: () => m.docs_wiki() },
	{ id: 'breeding', href: '/breeding', icon: 'tabler:flask', label: () => m.breeding() },
	{ id: 'editor', href: '/editor', icon: 'tabler:notebook', label: () => m.editor(), hideOnMobile: true },
	{ id: 'signal', href: '/signal', icon: 'local:rwby-rose', label: () => m.signal() }
];

export function activePublicNavId(pathname: string): string {
	let bestId = '';
	let bestLen = 0;
	for (const item of publicNavItems) {
		if (pathname === item.href || pathname.startsWith(`${item.href}/`)) {
			if (item.href.length > bestLen) {
				bestId = item.id;
				bestLen = item.href.length;
			}
		}
	}
	return bestId;
}
