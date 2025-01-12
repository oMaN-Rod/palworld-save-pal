<script lang="ts">
	import PalEdit from './components/pal/Edit.svelte';
	import PlayerEdit from './components/PlayerEdit.svelte';
	import PalBox from './components/palbox/PalBox.svelte';

	import { PlayerList } from '$components';
	import { Tooltip } from '$components/ui';
	import { EntryState, MessageType, type Pal, type Player } from '$types';
	import { SaveAll } from 'lucide-svelte';
	import { getAppState, getSocketState, getNavigationState, getToastState } from '$states';
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import { goto } from '$app/navigation';

	const appState = getAppState();
	const ws = getSocketState();
	const nav = getNavigationState();
	const toast = getToastState();

	interface ModifiedData {
		modified_pals?: Record<string, Pal>;
		modified_players?: Record<string, Player>;
	}

	async function handleSaveState() {
		let modifiedData: ModifiedData = {};
		const modifiedPals = Object.entries(appState.modifiedPals)
			.filter(([_, pal]) => pal.state === EntryState.MODIFIED)
			.map(([key, pal]) => {
				// @ts-ignore - We're removing the id from the pal object, no clue where it's coming from...
				const { id, ...palWithoutId } = pal;
				palWithoutId.state = EntryState.NONE;
				return [key, palWithoutId];
			});

		const modifiedPlayers = Object.entries(appState.modifiedPlayers)
			.filter(([_, player]) => player.state === EntryState.MODIFIED)
			.map(([id, player]) => {
				const { pals, ...playerWithoutPals } = player;
				playerWithoutPals.state = EntryState.NONE;
				return [id, playerWithoutPals];
			});

		if (modifiedPals.length === 0 && modifiedPlayers.length === 0) {
			console.log('No modifications to save');
			toast.add('No modifications to save', undefined, 'info');
			return;
		}

		if (modifiedPals.length > 0) {
			modifiedData.modified_pals = Object.fromEntries(modifiedPals);
		}

		if (modifiedPlayers.length > 0) {
			modifiedData.modified_players = Object.fromEntries(modifiedPlayers);
		}

		await goto('/loading');

		const data = {
			type: MessageType.UPDATE_SAVE_FILE,
			data: modifiedData
		};

		ws.send(JSON.stringify(data));

		appState.resetModified();

		const entityTypes = Object.keys(modifiedData).map((key) =>
			key.replace('modified', '').toLowerCase()
		);
		const entityMessage = entityTypes.join(' and ');
		ws.message = { type: MessageType.PROGRESS_MESSAGE, data: `Updating modified ${entityMessage}` };
	}

	$effect(() => {
		if (!appState.saveFile) {
			goto('/file');
		}
	});
</script>

<div class="flex h-full w-full overflow-hidden">
	<div class="relative h-full w-full">
		{#if appState.saveFile}
			<div class="absolute left-2 top-0 flex min-w-72 flex-shrink-0 flex-row">
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
			<Tabs
				listJustify="justify-center"
				bind:value={nav.activeTab}
				classes="flex h-full flex-col mt-4"
			>
				{#snippet list()}
					<div class="flex-shrink-0">
						<Tabs.Control value="player">Player</Tabs.Control>
						<Tabs.Control value="pal-box">Pal Box</Tabs.Control>
						<Tabs.Control value="pal">Pal</Tabs.Control>
					</div>
				{/snippet}
				{#snippet content()}
					<div class="flex-grow overflow-hidden">
						<Tabs.Panel value="player" classes="h-screen">
							<PlayerEdit />
						</Tabs.Panel>
						<Tabs.Panel value="pal-box" classes="h-screen">
							<PalBox />
						</Tabs.Panel>
						<Tabs.Panel value="pal" classes="h-screen">
							<PalEdit />
						</Tabs.Panel>
					</div>
				{/snippet}
			</Tabs>
		{/if}
	</div>
</div>
