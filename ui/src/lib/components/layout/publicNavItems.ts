import * as m from '$i18n/messages';
import BookOpen from '@lucide/svelte/icons/book-open';
import FlaskConical from '@lucide/svelte/icons/flask-conical';
import Map from '@lucide/svelte/icons/map';
import type { Component } from 'svelte';

export type PublicNavItem = {
	id: string;
	href: string;
	icon: Component;
	label: () => string;
};

export const publicNavItems: PublicNavItem[] = [
	{ id: 'map', href: '/map', icon: Map, label: () => m.map() },
	{ id: 'wiki', href: '/wiki', icon: BookOpen, label: () => m.docs_wiki() },
	{ id: 'breeding', href: '/breeding', icon: FlaskConical, label: () => m.breeding() }
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
