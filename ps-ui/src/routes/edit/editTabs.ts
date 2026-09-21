import type { ContextBarItem } from '$components/layout';
import * as m from '$i18n/messages';

export type EditTabContext = {
	hasSave: boolean;
	hasPlayer: boolean;
	hasDps: boolean;
	hasGps: boolean;
};

export type EditTab = {
	id: string;
	label: () => string;
	href: string;
	shortcut: string;
	available: (ctx: EditTabContext) => boolean;
	/** Reachable by shortcut but never rendered in the /edit strip. */
	shortcutOnly: boolean;
};

const hasSelectedPlayer = (ctx: EditTabContext) => ctx.hasSave && ctx.hasPlayer;

export const EDIT_TABS: EditTab[] = [
	{
		id: 'player',
		label: () => m.loadout(),
		href: '/edit/player',
		shortcut: 'KeyL',
		available: hasSelectedPlayer,
		shortcutOnly: false
	},
	{
		id: 'technologies',
		label: () => m.technology({ count: 2 }),
		href: '/edit/technologies',
		shortcut: 'KeyT',
		available: hasSelectedPlayer,
		shortcutOnly: false
	},
	{
		id: 'palbox',
		label: () => m.palbox(),
		href: '/edit/palbox',
		shortcut: 'KeyB',
		available: hasSelectedPlayer,
		shortcutOnly: false
	},
	{
		id: 'effigies',
		label: () => m.edit_effigies(),
		href: '/edit/effigies',
		shortcut: 'KeyF',
		available: hasSelectedPlayer,
		shortcutOnly: false
	},
	{
		id: 'dps',
		label: () => m.dps(),
		href: '/edit/dps',
		shortcut: 'KeyD',
		available: (ctx) => ctx.hasPlayer && ctx.hasDps,
		shortcutOnly: false
	},
	{
		id: 'guild',
		label: () => m.guild({ count: 1 }),
		href: '/edit/guild',
		shortcut: 'KeyG',
		available: (ctx) => ctx.hasPlayer,
		shortcutOnly: false
	},
	{
		id: 'missions',
		label: () => m.missions(),
		href: '/edit/missions',
		shortcut: 'KeyM',
		available: (ctx) => ctx.hasPlayer,
		shortcutOnly: false
	},
	{
		id: 'gps',
		label: () => m.gps(),
		href: '/gps',
		shortcut: 'KeyS',
		available: (ctx) => ctx.hasGps,
		shortcutOnly: true
	}
];

export function shortcutMap(): Record<string, string> {
	const map: Record<string, string> = {};
	for (const tab of EDIT_TABS) {
		map[tab.shortcut] = tab.id;
	}
	return map;
}

export function toContextItems(tabs: EditTab[], ctx: EditTabContext): ContextBarItem[] {
	return tabs.map((tab) => ({
		id: tab.id,
		label: tab.label(),
		href: tab.href,
		available: tab.available(ctx)
	}));
}
