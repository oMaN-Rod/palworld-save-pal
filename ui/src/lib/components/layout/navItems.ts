import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';
import { isWebBuild } from '$lib/utils/platform';
import type { AppState } from '$states';
import Blocks from '@lucide/svelte/icons/blocks';
import BookOpen from '@lucide/svelte/icons/book-open';
import Bug from '@lucide/svelte/icons/bug';
import ChevronsLeft from '@lucide/svelte/icons/chevrons-left';
import ChevronsRight from '@lucide/svelte/icons/chevrons-right';
import CircleX from '@lucide/svelte/icons/circle-x';
import Database from '@lucide/svelte/icons/database';
import FileHeart from '@lucide/svelte/icons/file-heart';
import FileText from '@lucide/svelte/icons/file-text';
import FlaskConical from '@lucide/svelte/icons/flask-conical';
import Folder from '@lucide/svelte/icons/folder';
import Globe from '@lucide/svelte/icons/globe';
import Info from '@lucide/svelte/icons/info';
import Layers from '@lucide/svelte/icons/layers';
import LayoutGrid from '@lucide/svelte/icons/layout-grid';
import Map from '@lucide/svelte/icons/map';
import NotebookPen from '@lucide/svelte/icons/notebook-pen';
import Pencil from '@lucide/svelte/icons/pencil';
import Puzzle from '@lucide/svelte/icons/puzzle';
import Save from '@lucide/svelte/icons/save';
import Server from '@lucide/svelte/icons/server';
import Settings from '@lucide/svelte/icons/settings';
import Wrench from '@lucide/svelte/icons/wrench';
import type { Component } from 'svelte';

export type NavSection = 'header' | 'tiles' | 'footer';

/** Sidebar grouping for `tiles` items. Determines which labelled cluster an item belongs to. */
export type NavGroup = 'main' | 'tools' | 'help';

export type NavAction = 'toggle-expanded' | 'save' | 'eject' | 'open-folder' | 'settings';

export type NavContext = {
	appState: AppState;
	desktop: boolean;
	expanded: boolean;
};

export type NavItem = {
	id: string;
	section: NavSection;
	/** Sidebar cluster for tiles. Defaults to `'main'`; ignored for header/footer items. */
	group?: NavGroup;
	/** Resolves the icon component, given runtime context (e.g. upload vs download). */
	icon: (ctx: NavContext) => Component;
	/** Expanded label text. Omit for icon-only tiles (e.g. the menu toggle). */
	label?: () => string;
	/** Tooltip text. Defaults to `label` when omitted. */
	title?: () => string;
	/** Navigation target for link tiles. Static, or resolved from runtime context. */
	href?: string | ((ctx: NavContext) => string);
	/** Stateful action for non-link tiles. Handled by Sidebar's action map. */
	action?: NavAction;
	/** Runtime visibility predicate. Visible when omitted. */
	visible?: (ctx: NavContext) => boolean;
};

/** Ordered sidebar groups. Labels resolve lazily so a locale switch re-reads them. */
export const navGroups: { id: NavGroup; label: () => string }[] = [
	{ id: 'main', label: () => m.nav_group_main() },
	{ id: 'tools', label: () => m.tools() },
	{ id: 'help', label: () => m.nav_group_help() }
];

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

	// --- tiles: main ---
	{
		id: 'files',
		section: 'tiles',
		group: 'main',
		icon: () => LayoutGrid,
		label: () => m.file({ count: 2 }),
		href: (ctx) => (ctx.desktop ? '/file' : '/upload')
	},
	{
		id: 'edit',
		section: 'tiles',
		group: 'main',
		icon: () => Pencil,
		label: () => m.edit(),
		href: '/edit'
	},
	{
		id: 'bulk',
		section: 'tiles',
		group: 'main',
		icon: () => Layers,
		label: () => m.bulk_actions(),
		href: '/bulk'
	},
	{
		id: 'map',
		section: 'tiles',
		group: 'main',
		icon: () => Map,
		label: () => m.map(),
		href: '/map'
	},
	{
		id: 'presets',
		section: 'tiles',
		group: 'main',
		icon: () => FileHeart,
		label: () => c.presets,
		href: '/presets'
	},

	// --- tiles: tools ---
	{
		id: 'blueprints',
		section: 'tiles',
		group: 'tools',
		icon: () => Blocks,
		label: () => 'Blueprints',
		href: '/blueprints'
	},
	{
		id: 'gps',
		section: 'tiles',
		group: 'tools',
		icon: () => Globe,
		label: () => m.gps(),
		href: '/gps',
		visible: (ctx) => ctx.appState.hasGpsAvailable
	},
	{
		id: 'ups',
		section: 'tiles',
		group: 'tools',
		icon: () => Database,
		label: () => m.ups(),
		href: '/ups'
	},
	{
		id: 'servers',
		section: 'tiles',
		group: 'tools',
		icon: () => Server,
		label: () => 'Servers',
		href: '/servers',
		// Server management drives Docker/native services the browser build cannot reach.
		visible: () => !isWebBuild
	},
	{
		id: 'editor',
		section: 'tiles',
		group: 'tools',
		icon: () => NotebookPen,
		label: () => m.editor(),
		href: '/editor'
	},
	{
		id: 'plugins',
		section: 'tiles',
		group: 'tools',
		icon: () => Puzzle,
		label: () => 'Plugins',
		href: '/plugins'
	},
	{
		id: 'debug',
		section: 'tiles',
		group: 'tools',
		icon: () => Bug,
		label: () => m.debug(),
		href: '/debug',
		visible: (ctx) => Boolean(ctx.appState.settings.debug_mode)
	},
	{
		id: 'breeding',
		section: 'tiles',
		group: 'tools',
		icon: () => FlaskConical,
		label: () => m.breeding(),
		href: '/breeding'
	},

	// --- tiles: help ---
	{
		id: 'tools',
		section: 'tiles',
		group: 'help',
		icon: () => Wrench,
		label: () => m.tools(),
		href: '/tools'
	},
	{
		id: 'docs',
		section: 'tiles',
		group: 'help',
		icon: () => FileText,
		label: () => m.docs(),
		href: '/docs'
	},
	{
		id: 'wiki',
		section: 'tiles',
		group: 'help',
		icon: () => BookOpen,
		label: () => m.docs_wiki(),
		href: '/wiki'
	},
	{
		id: 'about',
		section: 'tiles',
		group: 'help',
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
 * Function hrefs are resolved against the runtime context (e.g. Files → /file on
 * desktop vs /upload on the web build).
 */
export function activeNavId(pathname: string, ctx: NavContext): string {
	let bestId = '';
	let bestLen = 0;
	for (const item of navItems) {
		if (!item.href) continue;
		const href = typeof item.href === 'string' ? item.href : item.href(ctx);
		if (pathname === href || pathname.startsWith(`${href}/`)) {
			if (href.length > bestLen) {
				bestId = item.id;
				bestLen = href.length;
			}
		}
	}
	return bestId;
}
