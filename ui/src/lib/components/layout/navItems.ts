import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';
import { isWebBuild } from '$lib/utils/platform';
import type { AppState } from '$states';

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
	/** Resolves the Iconify icon name, given runtime context. */
	icon: (ctx: NavContext) => string;
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
		icon: (ctx) => (ctx.expanded ? 'tabler:chevrons-left' : 'tabler:chevrons-right'),
		title: () => m.toggle_entity({ entity: '' }),
		action: 'toggle-expanded'
	},
	{
		id: 'save',
		section: 'header',
		icon: () => 'tabler:device-floppy',
		label: () => c.save,
		action: 'save',
		visible: (ctx) => Boolean(ctx.appState.saveFile) && ctx.desktop
	},
	{
		id: 'eject',
		section: 'header',
		icon: () => 'tabler:circle-x',
		label: () => m.eject(),
		action: 'eject',
		visible: (ctx) => Boolean(ctx.appState.saveFile)
	},

	// --- tiles: main ---
	{
		id: 'overview',
		section: 'tiles',
		group: 'main',
		icon: () => 'tabler:layout-grid',
		label: () => m.overview(),
		// The overview is the file tab on both builds once a save is loaded;
		// on the web build without a save it points at the dropzone.
		href: (ctx) => (ctx.desktop || ctx.appState.saveFile ? '/overview' : '/upload')
	},
	{
		id: 'edit',
		section: 'tiles',
		group: 'main',
		icon: () => 'tabler:pencil',
		label: () => m.edit(),
		href: '/edit'
	},
	{
		id: 'registry',
		section: 'tiles',
		group: 'main',
		icon: () => 'tabler:stack-2',
		label: () => m.entity_registry(),
		href: '/registry'
	},
	{
		id: 'map',
		section: 'tiles',
		group: 'main',
		icon: () => 'tabler:map',
		label: () => m.map(),
		href: '/map'
	},
	{
		id: 'presets',
		section: 'tiles',
		group: 'main',
		icon: () => 'tabler:file-like',
		label: () => c.presets,
		href: '/presets'
	},

	// --- tiles: tools ---
	{
		id: 'blueprints',
		section: 'tiles',
		group: 'tools',
		icon: () => 'tabler:blocks',
		label: () => 'Blueprints',
		href: '/blueprints'
	},
	{
		id: 'gps',
		section: 'tiles',
		group: 'tools',
		icon: () => 'tabler:world',
		label: () => m.gps(),
		href: '/gps',
		visible: (ctx) => ctx.appState.hasGpsAvailable
	},
	{
		id: 'ups',
		section: 'tiles',
		group: 'tools',
		icon: () => 'tabler:database',
		label: () => m.ups(),
		href: '/ups'
	},
	{
		id: 'servers',
		section: 'tiles',
		group: 'tools',
		icon: () => 'tabler:server',
		label: () => 'Servers',
		href: '/servers',
		// Server management drives Docker/native services the browser build cannot reach.
		visible: () => !isWebBuild
	},
	{
		id: 'editor',
		section: 'tiles',
		group: 'tools',
		icon: () => 'tabler:notebook',
		label: () => m.editor(),
		href: '/editor'
	},
	{
		id: 'debug',
		section: 'tiles',
		group: 'tools',
		icon: () => 'tabler:bug',
		label: () => m.debug(),
		href: '/debug',
		visible: (ctx) => Boolean(ctx.appState.settings.debug_mode)
	},
	{
		id: 'breeding',
		section: 'tiles',
		group: 'tools',
		icon: () => 'tabler:flask',
		label: () => m.breeding(),
		href: '/breeding'
	},

	// --- tiles: help ---
	{
		id: 'tools',
		section: 'tiles',
		group: 'help',
		icon: () => 'tabler:tool',
		label: () => m.tools(),
		href: '/tools'
	},
	{
		id: 'docs',
		section: 'tiles',
		group: 'help',
		icon: () => 'tabler:file-text',
		label: () => m.docs(),
		href: '/docs'
	},
	{
		id: 'wiki',
		section: 'tiles',
		group: 'help',
		icon: () => 'tabler:book',
		label: () => m.docs_wiki(),
		href: '/wiki'
	},
	{
		id: 'about',
		section: 'tiles',
		group: 'help',
		icon: () => 'tabler:info-circle',
		label: () => m.about(),
		href: '/about'
	},

	// --- footer ---
	{
		id: 'open-folder',
		section: 'footer',
		icon: () => 'tabler:folder',
		label: () => m.open_folder(),
		action: 'open-folder',
		visible: (ctx) => ctx.desktop
	},
	{
		id: 'settings',
		section: 'footer',
		icon: () => 'tabler:settings',
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
