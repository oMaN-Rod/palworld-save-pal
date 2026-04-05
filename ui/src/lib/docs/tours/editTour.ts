import type { TourDefinition } from './types';

export const editTour: TourDefinition = {
	id: 'editing',
	title: 'Editing Save Data',
	description:
		'Learn how to navigate the edit section to modify players, Pals, guilds, and more. Requires a save file to be loaded.',
	route: '/edit',
	requiresSaveFile: true,
	steps: [
		{
			popover: {
				title: 'Edit Section',
				description:
					'This is the main editing area. From here you can modify players, Pals, guilds, technologies, and more.'
			}
		},
		{
			element: 'div[id="player-list"]',
			popover: {
				title: 'Player Selection',
				description:
					"Use the player dropdown at the top to select which player's data you want to edit."
			}
		},
		{
			element: 'div[id="inventory-panel"]',
			popover: {
				title: 'Inventory Panel',
				description:
					'This panel displays the player\'s inventory. You can view and manage items here.'
			}
		},
		{
			element: 'div[data-scope="tabs"]',
			popover: {
				title: 'Inventory/Key Item Tabs',
				description:
					'These tabs allow you to switch between the inventory and key items panels.'
			}
		},
		{
			element: 'button[data-value="key_items"]',
			popover: {
				title: 'Key Items Tab',
				description:
					'Click this tab to view and edit key items, which are important for quests and progression.'
			}
		},
		{
			element: 'div[id="key-items-panel"]',
			popover: {
				title: 'Key Items Panel',
				description:
					'This panel displays the player\'s key items. You can view and manage key items here.'
			}
		},
		{
			element: 'div[id="weapon-equip"]',
			popover: {
				title: 'Equipped Weapons',
				description:
					'This section shows the player\'s currently equipped weapons. You can view and modify weapons here.'
			}
		},
		{
			element: 'div[id="accessory-equip"]',
			popover: {
				title: 'Equipped Accessories',
				description:
					'This section shows the player\'s currently equipped accessories. You can view and modify accessories here.'
			}
		},
		{
			element: 'div[id="gear-equip"]',
			popover: {
				title: 'Equipped Gear',
				description:
					'This section shows the player\'s currently equipped gear. You can view and modify head, body, shield, glider, and sphere module gear here.'
			}
		},
		{
			popover: {
				title: 'Keyboard Shortcuts',
				description:
					'Use keyboard shortcuts to quickly navigate: L (Loadout), T (Technologies), B (Palbox), D (DPS), G (Guild), M (Missions), P (Pal).'
			}
		}
	]
};
