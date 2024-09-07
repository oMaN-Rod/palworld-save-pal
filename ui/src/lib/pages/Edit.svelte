<script lang="ts">
	import PalEdit from './PalEdit.svelte';
	import PlayerEdit from './PlayerEdit.svelte';

	import { Drawer, PlayerList, PalList } from '$components';
	import { Tooltip } from '$components/ui';
	import { MessageType, type Pal, type Player } from '$types';
	import { SaveAll } from 'lucide-svelte';
	import { getAppState, getSocketState, getNavigationState } from '$states';
	import { activeSkillsData, passiveSkillsData } from '$lib/data';
	import { Tabs } from '@skeletonlabs/skeleton-svelte';

	const appState = getAppState();
	const ws = getSocketState();
	const nav = getNavigationState();

	let palLevel: string = $state('');
	let palLevelClass: string = $state('');
	let group = $state('player');

	interface ModifiedData {
		modified_pals?: Record<string, Pal>;
		modified_players?: Record<string, Player>;
	}

	function handleSaveState() {
		let modifiedData: ModifiedData = {};

		if (Object.keys(appState.modifiedPals).length > 0) {
			modifiedData.modified_pals = Object.fromEntries(
				Object.entries(appState.modifiedPals).map(([key, pal]) => {
					// @ts-ignore - We're removing the id from the pal object, no clue where it's coming from...
					const { id, ...palWithoutId } = pal;
					return [key, palWithoutId];
				})
			);
		}

		if (Object.keys(appState.modifiedPlayers).length > 0) {
			modifiedData.modified_players = Object.fromEntries(
				Object.entries(appState.modifiedPlayers).map(([id, player]) => {
					const { pals, ...playerWithoutPals } = player;
					return [id, playerWithoutPals];
				})
			);
		}

		if (Object.keys(modifiedData).length === 0) {
			console.log('No modifications to save');
			return;
		}

		const data = {
			type: MessageType.UPDATE_SAVE_FILE,
			data: modifiedData
		};

		ws.send(JSON.stringify(data));

		const entityTypes = Object.keys(modifiedData).map((key) =>
			key.replace('modified', '').toLowerCase()
		);
		const entityMessage = entityTypes.join(' and ');
		ws.message = { type: MessageType.PROGRESS_MESSAGE, data: `Updating modified ${entityMessage}` };
		nav.activePage = 'Loading';
	}

	$effect(() => {
		const loadSkills = async () => {
			await activeSkillsData.getActiveSkills();
			await passiveSkillsData.getPassiveSkills();
		};
		loadSkills();
	});

	$effect(() => {
		if (appState.selectedPlayer && appState.selectedPal) {
			palLevel =
				appState.selectedPlayer.level < appState.selectedPal.level
					? appState.selectedPlayer.level.toString()
					: appState.selectedPal.level.toString();
			palLevelClass =
				appState.selectedPlayer.level < appState.selectedPal.level ? 'text-error-500' : '';
		}
	});
</script>

<div class="flex h-full w-full overflow-hidden">
	<div class="grid h-full w-full" style="grid-template-columns: var(--drawer-width) 1fr;">
		{#if appState.saveFile}
			<Drawer initiallyExpanded={true}>
				<div class="flex h-full flex-col">
					<div class="flex flex-shrink-0 flex-row">
						<PlayerList />
						{#if (appState.modifiedPals && Object.keys(appState.modifiedPals).length > 0) || (appState.modifiedPlayers && Object.keys(appState.modifiedPlayers).length > 0)}
							<div class="mr-0 flex items-end justify-end pb-2 pr-0">
								<Tooltip>
									<button class="btn" onclick={handleSaveState}>
										<SaveAll class="text-primary-500 mr-2" size="32" />
									</button>
									{#snippet popup()}
										<span>Save all changes</span>
									{/snippet}
								</Tooltip>
							</div>
						{/if}
					</div>
					{#if appState.selectedPlayer}
						<div class="overflow-hideen flex-grow">
							<PalList />
						</div>
					{/if}
				</div>
			</Drawer>
			<Tabs listJustify="justify-center" bind:value={group} class="flex h-full flex-col">
				{#snippet list()}
					<div class="flex-shrink-0">
						<Tabs.Control value="player">Player</Tabs.Control>
						<Tabs.Control value="pal">Pal</Tabs.Control>
					</div>
				{/snippet}
				{#snippet content()}
					<div class="flex-grow overflow-hidden">
						<Tabs.Panel value="player" class="h-full">
							<PlayerEdit />
						</Tabs.Panel>
						<Tabs.Panel value="pal" class="h-full">
							<PalEdit />
						</Tabs.Panel>
					</div>
				{/snippet}
			</Tabs>
		{/if}
	</div>
</div>
