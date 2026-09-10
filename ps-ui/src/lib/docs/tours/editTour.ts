import { getAppState, getPalEditorState } from '$states';
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
			checkpoint: {
				condition: () => !!getAppState().selectedPlayerUid
			},
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
			element: 'nav[id="quick-actions"]',
			popover: {
				title: 'Quick Actions',
				description:
					'This section provides quick access to common actions such as sorting inventory, filling containers, and clearing items.'
			}
		},
		{
			element: 'div[data-testid="tabs-list"]',
			popover: {
				title: 'Inventory/Key Item Tabs',
				description:
					'These tabs allow you to switch between the inventory and key items panels.'
			}
		},
		{
			element: 'button[data-value="key_items"]',
			checkpoint: { selector: 'button[data-value="key_items"]' },
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
					'This panel displays the player\'s key items.'
			}
		},
		{
			element: 'nav[id="quick-actions"]',
			popover: {
				title: 'Quick Actions',
				description:
					'Quick actions are also available in the key items tab for sorting, adding, and clearing key items.'
			}
		},
		{
			element: 'div[id="weapon-equip"]',
			popover: {
				title: 'Equipped Weapons',
				description:
					'This section shows the player\'s currently equipped weapons.'
			}
		},
		{
			element: 'div[id="accessory-equip"]',
			popover: {
				title: 'Equipped Accessories',
				description:
					'This section shows the player\'s currently equipped accessories.'
			}
		},
		{
			element: 'div[id="gear-equip"]',
			popover: {
				title: 'Equipped Gear',
				description:
					'This section shows the player\'s currently equipped head, body, shield, glider, and sphere module gear.'
			}
		},
		{
			element: 'div[id="food-equip"]',
			popover: {
				title: 'Equipped Food',
				description:
					'This section shows the player\'s currently equipped food items.'
			}
		},
		{
			element: 'div[id="player-level"]',
			popover: {
				title: 'Player Level',
				description:
					'View and edit the player\'s level progression.'
			}
		},
		{
			element: 'button[id="player-nickname"]',
			popover: {
				title: 'Player Nickname',
				description:
					'Edit the player\'s nickname.'
			}
		},
		{
			element: 'button[id="player-heal-hp"]',
			popover: {
				title: 'Heal Player HP',
				description:
					'Fully restore the player\'s HP.'
			}
		},
		{
			element: 'button[id="player-fill-stomach"]',
			popover: {
				title: 'Fill Player Stomach',
				description:
					'Fully fill the player\'s stomach.'
			}
		},
		{
			element: 'div[id="player-stats"]',
			popover: {
				title: 'Player Stats',
				description:
					'View and edit the player\'s stats such as health, stamina, attack, defense, workspeed and carry weight.'
			}
		},
		{
			element: 'button[id="max-player-stats"]',
			popover: {
				title: 'Max Player Stats',
				description:
					'Set all player stats to their maximum values.'
			}
		},
		{
			element: 'div[id="player-presets-control"]',
			checkpoint: { selector: 'div[id="player-presets-control"]' },
			popover: {
				title: 'Player Presets',
				description:
					'Click here to expand the player presets panel, where you can save and load presets of player data.'
			}
		},
		{
			element: 'div[id="player-presets"]',
			popover: {
				title: 'Player Presets',
				description:
					'This allows you to manage player presets, which are saved configurations of player load outs that you can quickly apply.'
			}
		},
		{
			element: 'div[id="player-presets-quick-actions"]',
			popover: {
				title: 'Player Presets Quick Actions',
				description:
					'This section provides quick access to common actions for managing player presets, such as adding, applying, and deleting presets. Note, select a preset to see the apply and delete buttons.'
			}
		},
		{
			element: 'div[id="player-tabs"]',
			popover: {
				title: 'Player Edit Tabs',
				description:
					'Use these tabs to navigate between different sections of player data, such as loadout, technologies, palbox, dps, guild, missions, and pals.'
			}
		},
		{
			element: 'button[id="technology-tab"]',
			checkpoint: {
				selector: 'button[id="technology-tab"]',
				advanceDelayMs: 500
			},
			popover: {
				title: 'Technologies Tab',
				description:
					'Click this tab to view and edit the player\'s unlocked technologies.'
			}
		},
		{
			element: 'button[id="tech-points"]',
			popover: {
				title: 'Technology Points',
				description:
					'Your available technology points. Click to edit. These are spent to unlock regular technologies.'
			}
		},
		{
			element: 'button[id="ancient-tech-points"]',
			popover: {
				title: 'Ancient Technology Points',
				description:
					'Your available ancient technology points. Click to edit. These are earned from defeating bosses and spent to unlock ancient technologies.'
			}
		},
		{
			element: 'div[id="tech-bulk-actions"]',
			popover: {
				title: 'Bulk Actions',
				description:
					'Quick actions to lock all technologies or unlock everything at once.'
			}
		},
		{
			element: 'button[id="tech-lock-all"]',
			popover: {
				title: 'Lock All',
				description:
					'Reset all unlocked technologies back to locked. Use this if you want to start fresh.'
			}
		},
		{
			element: 'button[id="tech-unlock-all"]',
			popover: {
				title: 'Unlock All',
				description:
					'Unlock every technology in the game at once.'
			}
		},
		{
			element: 'div[id="tech-grid"]',
			popover: {
				title: 'Technology Tree',
				description:
					'Technologies are grouped by the player level they unlock at. Click any technology to toggle it unlocked or locked. Hover a technology to preview what it unlocks.'
			}
		},
		{
			element: 'button[id="palbox-tab"]',
			checkpoint: {
				selector: 'button[id="palbox-tab"]',
				advanceDelayMs: 500
			},
			popover: {
				title: 'Palbox Tab',
				description: "Click this tab to view and edit the player's Palbox and party."
			}
		},
		{
			element: 'nav[id="palbox-toolbar"]',
			popover: {
				title: 'Palbox Toolbar',
				description:
					'Quick actions for the Palbox. Additional actions appear here when one or more Pals are selected (Ctrl+Click a Pal to select it).'
			}
		},
		{
			element: 'button[id="palbox-add-pal"]',
			popover: {
				title: 'Add New Pal',
				description: 'Add a new Pal directly to the Palbox.'
			}
		},
		{
			element: 'button[id="palbox-add-all"]',
			popover: {
				title: 'Add All Pals',
				description: 'Fill the Palbox with one of every Pal in the game.'
			}
		},
		{
			element: 'button[id="palbox-select-all"]',
			popover: {
				title: 'Select All',
				description:
					'Select every Pal in the current view. Ctrl+Click this to also include the party.'
			}
		},
		{
			element: 'button[id="palbox-heal-all"]',
			popover: {
				title: 'Heal All',
				description:
					'Fully heal every Pal in the Palbox — restores HP, sanity, stomach, and clears sickness.'
			}
		},
		{
			element: 'div[id="palbox-filters"]',
			popover: {
				title: 'Filter & Sort',
				description:
					'Expand this panel to search by name, sort by level/name/paldeck index, and filter by element, alpha, lucky, human, predator, oil rig, or summoned.'
			}
		},
		{
			element: 'div[id="palbox-party"]',
			popover: {
				title: 'Party',
				description:
					"The player's active party of up to 5 Pals. You can add, move to Palbox, clone, or delete Pals from here."
			}
		},
		{
			element: 'div[id="palbox-pager"]',
			popover: {
				title: 'Box Pager',
				description:
					'Navigate between Palbox pages. Each box holds 30 Pals. Use Q/E or arrow keys to page as well.'
			}
		},
		{
			element: 'div[id="palbox-grid"]',
			popover: {
				title: 'Palbox Grid',
				description:
					'The Pals in the current box. Click a Pal to open it in the editor. Ctrl+Click to select one or more for bulk actions.'
			}
		},
		{
			element: 'div[id="palbox-stats"]',
			popover: {
				title: 'Palbox Stats',
				description:
					'Aggregate stats for the current box — totals, element breakdown, and other useful numbers at a glance.'
			}
		},
		{
			element: 'button[id="dps-tab"]',
			checkpoint: {
				selector: 'button[id="dps-tab"]',
				advanceDelayMs: 500
			},
			popover: {
				title: 'DPS Tab',
				description:
					"Click this tab to view and edit the Dimensional Pal Storage. This tab only appears if your save has DPS data — otherwise you can skip ahead."
			}
		},
		{
			element: 'div[id="dps-toolbar"]',
			popover: {
				title: 'DPS Toolbar',
				description:
					'Quick actions for Dimensional Pal Storage. Additional actions (apply preset, clone to UPS, delete, clear) appear here when one or more Pals are selected.'
			}
		},
		{
			element: 'button[id="dps-add-all"]',
			popover: {
				title: 'Fill DPS',
				description: 'Bulk-fill the Dimensional Pal Storage with Pals from a preset or template.'
			}
		},
		{
			element: 'button[id="dps-select-all"]',
			popover: {
				title: 'Select All',
				description: 'Select every Pal in the current filtered view for bulk actions.'
			}
		},
		{
			element: 'div[id="dps-filters"]',
			popover: {
				title: 'Filter & Sort',
				description:
					'Expand this panel to search and filter DPS Pals by name, level, paldeck index, element, alpha, lucky, human, predator, oil rig, or summoned.'
			}
		},
		{
			element: 'div[id="dps-pager"]',
			popover: {
				title: 'DPS Pager',
				description:
					'Navigate DPS pages. DPS holds up to 9,600 Pals across many pages. Use Q/E or arrow keys to page as well.'
			}
		},
		{
			element: 'div[id="dps-grid"]',
			popover: {
				title: 'DPS Grid',
				description:
					'The Pals in the current DPS page. Click a Pal to open it in the editor. Ctrl+Click to select one or more for bulk actions.'
			}
		},
		{
			element: 'div[id="dps-stats"]',
			popover: {
				title: 'DPS Stats',
				description:
					'Aggregate stats for the DPS — totals, element breakdown, and other summary numbers.'
			}
		},
		{
			element: 'button[id="guild-tab"]',
			checkpoint: {
				selector: 'button[id="guild-tab"]',
				advanceDelayMs: 500
			},
			popover: {
				title: 'Guild Tab',
				description:
					"Click this tab to view and edit the player's guild — members, bases, storage, chest, and lab research."
			}
		},
		{
			element: 'button[id="guild-name"]',
			popover: {
				title: 'Guild Name',
				description: 'Click to rename the guild.'
			}
		},
		{
			element: 'button[id="guild-level"]',
			popover: {
				title: 'Basecamp Level',
				description: 'Click to adjust the basecamp level (1–30).'
			}
		},
		{
			element: 'button[id="guild-delete"]',
			popover: {
				title: 'Delete Guild',
				description:
					'Permanently delete the entire guild. This is irreversible — make a backup first.'
			}
		},
		{
			element: 'button[id="guild-base-name"]',
			popover: {
				title: 'Base Name',
				description: 'Click to rename the currently selected base.'
			}
		},
		{
			element: 'div[id="guild-pager"]',
			popover: {
				title: 'Base Pager',
				description:
					"Navigate between the guild's bases. Use Q/E or arrow keys to page as well."
			}
		},
		{
			element: 'nav[id="guild-tabs"]',
			popover: {
				title: 'Guild Sub-tabs',
				description:
					'Switch between Pals, Storage, Chest, and Lab views for the current base. The tour will walk through each next.'
			}
		},
		{
			element: 'div[id="guild-pals-toolbar"]',
			popover: {
				title: 'Base Pals Toolbar',
				description:
					'Actions for Pals in the current base. Additional actions (preset, heal selected, delete, clear) appear when one or more Pals are selected.'
			}
		},
		{
			element: 'button[id="guild-pals-add"]',
			popover: {
				title: 'Add Pal to Base',
				description: 'Add a new Pal to the currently selected base.'
			}
		},
		{
			element: 'button[id="guild-pals-select-all"]',
			popover: {
				title: 'Select All (Base)',
				description: 'Select every Pal in the current base for bulk actions.'
			}
		},
		{
			element: 'button[id="guild-pals-heal-all"]',
			popover: {
				title: 'Heal All (Base)',
				description: 'Fully heal every Pal in the current base.'
			}
		},
		{
			element: 'div[id="guild-pals-grid"]',
			popover: {
				title: 'Base Pals',
				description:
					'Pals assigned to this base. Click a Pal to open the editor. Ctrl+Click to select one or more for bulk actions.'
			}
		},
		{
			element: 'button[id="guild-tab-storage"]',
			checkpoint: {
				selector: 'button[id="guild-tab-storage"]',
				advanceDelayMs: 200
			},
			popover: {
				title: 'Storage Tab',
				description: "Click Storage to browse this base's storage containers and their contents."
			}
		},
		{
			element: 'div[id="guild-storage-content"]',
			popover: {
				title: 'Base Storage',
				description:
					'Left: list of storage containers in this base. Select one to see its items on the right. You can search the full base inventory to locate items.'
			}
		},
		{
			element: 'button[id="guild-tab-chest"]',
			checkpoint: {
				selector: 'button[id="guild-tab-chest"]',
				advanceDelayMs: 200
			},
			popover: {
				title: 'Guild Chest Tab',
				description: "Click Chest to view and edit the shared guild chest."
			}
		},
		{
			element: 'div[id="guild-chest-content"]',
			popover: {
				title: 'Guild Chest',
				description:
					'The guild chest is shared across all members. Right-click slots to copy/paste items, Ctrl+Middle-click to clear a slot.'
			}
		},
		{
			element: 'button[id="guild-tab-lab"]',
			checkpoint: {
				selector: 'button[id="guild-tab-lab"]',
				advanceDelayMs: 200
			},
			popover: {
				title: 'Lab Tab',
				description: "Click Lab to view and edit the guild's research progress."
			}
		},
		{
			element: 'div[id="guild-lab-content"]',
			popover: {
				title: 'Lab Research',
				description:
					'Unlock or lock lab research items by category. Useful for testing late-game content or resetting research progress.'
			}
		},
		{
			element: 'button[id="missions-tab"]',
			checkpoint: {
				selector: 'button[id="missions-tab"]',
				advanceDelayMs: 500
			},
			popover: {
				title: 'Missions Tab',
				description:
					"Click this tab to view and edit the player's mission progress — both Main and Sub missions."
			}
		},
		{
			element: 'div[id="missions-tabs"]',
			popover: {
				title: 'Main / Sub Tabs',
				description:
					'Switch between Main missions (story) and Sub missions (side content). Bulk actions apply to whichever tab is active.'
			}
		},
		{
			element: 'div[id="missions-actions"]',
			popover: {
				title: 'Mission Bulk Actions',
				description:
					'Three bulk actions for the active tab: mark all current as complete, clear all current, or clear all completed missions.'
			}
		},
		{
			element: 'button[id="missions-mark-current-complete"]',
			popover: {
				title: 'Mark All Current Complete',
				description:
					'Move every currently-active mission on this tab to the completed list. Useful for skipping ahead or debugging progression.'
			}
		},
		{
			element: 'button[id="missions-clear-current"]',
			popover: {
				title: 'Clear All Current',
				description:
					'Remove every currently-active mission on this tab. Lets you reset in-progress missions.'
			}
		},
		{
			element: 'button[id="missions-clear-completed"]',
			popover: {
				title: 'Clear All Completed',
				description:
					'Remove every completed mission on this tab. Useful for re-triggering previously completed content.'
			}
		},
		{
			element: 'div[id="missions-list"]',
			popover: {
				title: 'Mission List',
				description:
					'Every mission for this tab grouped by current and completed. Click a mission to view its details. Each row has inline buttons to mark complete or clear individually.'
			}
		},
		{
			element: 'div[id="missions-details"]',
			popover: {
				title: 'Mission Details',
				description:
					'Detailed information for the selected mission — objectives, rewards, and related data.'
			}
		},
		{
			checkpoint: {
				condition: () => !!getAppState().selectedPal && getPalEditorState().isOpen,
				advanceDelayMs: 600
			},
			popover: {
				title: 'Select a Pal',
				description:
					'To edit an individual Pal, navigate to the Palbox or Party and click any Pal. The editor will open automatically — the tour will continue once a Pal is loaded.'
			}
		},
		{
			element: 'div[id="pal-header"]',
			popover: {
				title: 'Pal Header',
				description:
					'Basic info for the selected Pal — name/nickname, level and experience, character ID, and quick-edit controls.'
			}
		},
		{
			element: 'div[id="pal-active-skills"]',
			popover: {
				title: 'Active Skills',
				description:
					'Up to 3 active skills the Pal can use in combat. Use the header buttons to manage learned skills, save the current loadout as a preset, or add a new skill.'
			}
		},
		{
			element: 'div[id="pal-passive-skills"]',
			popover: {
				title: 'Passive Skills',
				description:
					'Up to 4 passive skills that buff the Pal permanently. Use the header buttons to save as a preset or add a new passive.'
			}
		},
		{
			element: 'div[id="pal-skill-presets"]',
			popover: {
				title: 'Skill Presets',
				description:
					'Apply a saved active or passive skill preset to this Pal with a single click. Useful for quickly outfitting multiple Pals.'
			}
		},
		{
			element: 'div[id="pal-work-suitability"]',
			popover: {
				title: 'Work Suitability',
				description:
					"The Pal's work suitabilities (Kindling, Watering, Planting, etc.). Use the max button in the header to cap each suitability at 5."
			}
		},
		{
			element: 'div[id="pal-image"]',
			popover: {
				title: 'Pal Portrait',
				description: "Portrait of the selected Pal. Hover to see the Pal's in-game description."
			}
		},
		{
			element: 'div[id="pal-status"]',
			popover: {
				title: 'Status',
				description:
					'Current HP, sanity, stomach, and other live status values. Edit these to revive, feed, or adjust mood.'
			}
		},
		{
			element: 'div[id="pal-stats"]',
			popover: {
				title: 'Stats',
				description:
					"Computed stats derived from the Pal's level, IVs, souls, and passive skills. These update automatically when you change the inputs below."
			}
		},
		{
			element: 'div[id="pal-talents"]',
			popover: {
				title: 'Talents / IVs',
				description:
					'Individual Values for HP, attack, and defense. Use the max button in the header to max all IVs at once. Cheat mode (in settings) unlocks values up to 255.'
			}
		},
		{
			element: 'div[id="pal-souls"]',
			popover: {
				title: 'Souls',
				description:
					'Soul upgrades for HP, attack, defense, and craft speed. Max button maxes all souls. Cheat mode unlocks values up to 255.'
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
