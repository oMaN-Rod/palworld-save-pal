import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';
import { isWebBuild } from '$lib/utils/platform';
import type { AppState } from '$states';

export type NavSection = 'header' | 'tiles' | 'footer';

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
	group?: NavGroup;
	/** Iconify icon name, e.g. `tabler:map`. */
	icon: (ctx: NavContext) => string;
	label?: () => string;
	title?: () => string;
	href?: string | ((ctx: NavContext) => string);
	action?: NavAction;
	visible?: (ctx: NavContext) => boolean;
};

// Labels are functions, not strings, so a locale switch re-reads them.
export const navGroups: { id: NavGroup; label: () => string }[] = [
	{ id: 'main', label: () => m.nav_group_main() },
	{ id: 'tools', label: () => m.tools() },
	{ id: 'help', label: () => m.nav_group_help() }
];

export const navItems: NavItem[] = [
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
		id: 'bulk',
		section: 'tiles',
		group: 'main',
		icon: () => 'tabler:stack-2',
		label: () => m.bulk_actions(),
		href: '/bulk'
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
		// 'tabler:server' management drives Docker/native services the browser build cannot reach.
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
		id: 'plugins',
		section: 'tiles',
		group: 'tools',
		icon: () => 'tabler:puzzle',
		label: () => 'Plugins',
		href: '/plugins'
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

// Longest `href` match wins on a route-segment boundary, so `/editor` beats `/edit`.
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
