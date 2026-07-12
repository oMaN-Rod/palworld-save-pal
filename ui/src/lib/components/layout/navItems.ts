import type { Component } from 'svelte';
import {
	BookOpen,
	Bug,
	ChevronsLeft,
	ChevronsRight,
	CircleX,
	Database,
	Download,
	File,
	FileHeart,
	Folder,
	Globe,
	Info,
	Layers,
	Map,
	NotebookPen,
	Pencil,
	Save,
	Server,
	Settings,
	Upload,
	Wrench
} from '@lucide/svelte';
import type { AppState } from '$states';
import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';

export type NavSection = 'header' | 'tiles' | 'footer';

export type NavAction = 'toggle-expanded' | 'save' | 'eject' | 'open-folder' | 'settings';

export type NavContext = {
	appState: AppState;
	desktop: boolean;
	expanded: boolean;
};

export type NavItem = {
	id: string;
	section: NavSection;
	/** Resolves the icon component, given runtime context (e.g. upload vs download). */
	icon: (ctx: NavContext) => Component;
	/** Expanded label text. Omit for icon-only tiles (e.g. the menu toggle). */
	label?: () => string;
	/** Tooltip text. Defaults to `label` when omitted. */
	title?: () => string;
	/** Navigation target for link tiles. */
	href?: string;
	/** Stateful action for non-link tiles. Handled by NavBar's action map. */
	action?: NavAction;
	/** Runtime visibility predicate. Visible when omitted. */
	visible?: (ctx: NavContext) => boolean;
};

export const navItems: NavItem[] = [
	// --- header ---
	{
		id: 'menu',
		section: 'header',
		icon: (ctx) => (ctx.expanded ? ChevronsLeft : ChevronsRight),
		title: () => m.toggle_entity({ entity: '' }),
		action: 'toggle-expanded'
	},
	{
		id: 'save',
		section: 'header',
		icon: () => Save,
		label: () => c.save,
		action: 'save',
		visible: (ctx) => Boolean(ctx.appState.saveFile) && ctx.desktop
	},
	{
		id: 'eject',
		section: 'header',
		icon: () => CircleX,
		label: () => m.eject(),
		action: 'eject',
		visible: (ctx) => Boolean(ctx.appState.saveFile)
	},

	// --- tiles ---
	{
		id: 'edit',
		section: 'tiles',
		icon: () => Pencil,
		label: () => m.edit(),
		href: '/edit'
	},
	{
		id: 'bulk',
		section: 'tiles',
		icon: () => Layers,
		label: () => m.bulk_actions(),
		href: '/bulk'
	},
	{
		id: 'file',
		section: 'tiles',
		icon: () => File,
		label: () => m.file({ count: 2 }),
		href: '/file',
		visible: (ctx) => ctx.desktop
	},
	{
		id: 'upload',
		section: 'tiles',
		icon: (ctx) => (ctx.appState.saveFile ? Download : Upload),
		label: () => m.transfer({ count: 1 }),
		href: '/upload',
		visible: (ctx) => !ctx.desktop
	},
	{
		id: 'map',
		section: 'tiles',
		icon: () => Map,
		label: () => m.map(),
		href: '/worldmap'
	},
	{
		id: 'presets',
		section: 'tiles',
		icon: () => FileHeart,
		label: () => c.presets,
		href: '/presets'
	},
	{
		id: 'gps',
		section: 'tiles',
		icon: () => Globe,
		label: () => m.gps(),
		href: '/gps',
		visible: (ctx) => ctx.appState.hasGpsAvailable
	},
	{
		id: 'ups',
		section: 'tiles',
		icon: () => Database,
		label: () => m.ups(),
		href: '/ups'
	},
	{
		id: 'debug',
		section: 'tiles',
		icon: () => Bug,
		label: () => m.debug(),
		href: '/debug',
		visible: (ctx) => Boolean(ctx.appState.settings.debug_mode)
	},
	{
		id: 'servers',
		section: 'tiles',
		icon: () => Server,
		label: () => 'Servers',
		href: '/servers'
	},
	{
		id: 'editor',
		section: 'tiles',
		icon: () => NotebookPen,
		label: () => m.editor(),
		href: '/editor'
	},
	{
		id: 'tools',
		section: 'tiles',
		icon: () => Wrench,
		label: () => m.tools(),
		href: '/tools'
	},
	{
		id: 'docs',
		section: 'tiles',
		icon: () => BookOpen,
		label: () => m.docs(),
		href: '/docs'
	},
	{
		id: 'about',
		section: 'tiles',
		icon: () => Info,
		label: () => m.about(),
		href: '/about'
	},

	// --- footer ---
	{
		id: 'open-folder',
		section: 'footer',
		icon: () => Folder,
		label: () => m.open_folder(),
		action: 'open-folder',
		visible: (ctx) => ctx.desktop
	},
	{
		id: 'settings',
		section: 'footer',
		icon: () => Settings,
		label: () => m.settings(),
		action: 'settings'
	}
];

/**
 * Resolves the active nav id for a given pathname by longest-matching `href`
 * on a route-segment boundary. The longest match wins, so `/editor` beats `/edit`.
 */
export function activeNavId(pathname: string): string {
	let bestId = '';
	let bestLen = 0;
	for (const item of navItems) {
		if (!item.href) continue;
		if (pathname === item.href || pathname.startsWith(`${item.href}/`)) {
			if (item.href.length > bestLen) {
				bestId = item.id;
				bestLen = item.href.length;
			}
		}
	}
	return bestId;
}