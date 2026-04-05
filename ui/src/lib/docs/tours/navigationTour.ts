import type { TourDefinition } from './types';

export const navigationTour: TourDefinition = {
	id: 'navigation',
	title: 'App Navigation',
	description: 'Learn how to navigate around Palworld Save Pal and find key features.',
	route: '/docs/tours',
	requiresSaveFile: false,
	steps: [
		{
			popover: {
				title: 'Welcome to Palworld Save Pal',
				description:
					"This tour will walk you through the main areas of the application. Let's explore the navigation bar."
			}
		},
		{
			element: 'a[href="/edit"]',
			popover: {
				title: 'Edit Section',
				description:
					"Edit players, Pals, guilds, technologies, and more. You'll need a save file loaded first."
			}
		},
		{
			element: 'a[href="/file"], a[href="/upload"]',
			popover: {
				title: 'File Management',
				description:
					'Load save files here. In desktop mode this opens a folder picker. In web mode, you upload files directly.'
			}
		},
		{
			element: 'a[href="/worldmap"]',
			popover: {
				title: 'World Map',
				description:
					'An interactive map showing player locations, bases, and points of interest from your save file.'
			}
		},
		{
			element: 'a[href="/presets"]',
			popover: {
				title: 'Presets',
				description:
					'Manage and apply presets for your save files, allowing for quick configuration changes.'
			}
		},
		{
			element: 'a[href="/ups"]',
			popover: {
				title: 'Universal Pal Storage (UPS)',
				description:
					'View and manage your UPS Pals, which can be shared across all your saves.'
			}
		},
		{
			element: 'a[href="/debug"]',
			popover: {
				title: 'Debug',
				description:
					'Access debugging tools for inspecting game data.'
			}
		},
		{
			element: 'a[href="/servers"]',
			popover: {
				title: 'Server Manager',
				description:
					'Manage dedicated Palworld servers. Install, configure, start, and stop servers from here.'
			}
		},
		{
			element: 'a[href="/editor"]',
			popover: {
				title: 'Editor',
				description:
					'Load, convert, and edit save files directly in a text-based editor. Useful for advanced users who want to make bulk changes or fix issues.'
			}
		},
		{
			element: 'a[href="/tools"]',
			popover: {
				title: 'Tools',
				description: 'Utilities like save file format conversion between Steam and GamePass.'
			}
		},
		{
			element: 'a[href="/docs"]',
			popover: {
				title: 'Documentation',
				description:
					"You're here! Browse the game data wiki, read guides, and take interactive tours like this one."
			}
		},
		{
			element: 'button[title="Open Folder"]',
			popover: {
				title: 'Open Folder',
				description:
					'Open default folder locations.'
			}
		},
		{
			element: 'button[title="Settings"]',
			popover: {
				title: 'Settings',
				description:
					'Access application settings, configure preferences, and manage your account.'
			}
		}
	]
};
