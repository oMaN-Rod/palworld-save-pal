<script lang="ts">
	import PalEdit from './components/pal/Edit.svelte';
	import PlayerEdit from './components/PlayerEdit.svelte';
	import PalBox from './components/palbox/PalBox.svelte';

	import { PlayerList } from '$components';
	import { Tooltip } from '$components/ui';
	import { type Pal, type Player } from '$types';
	import { SaveAll } from 'lucide-svelte';
	import { getAppState, getNavigationState } from '$states';
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import { goto } from '$app/navigation';
	import Guilds from './components/guilds/Guilds.svelte';

	const appState = getAppState();
	const nav = getNavigationState();

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
							<button class="btn" onclick={appState.saveState}>
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
						<Tabs.Control value="guilds">Bases</Tabs.Control>
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
						<Tabs.Panel value="guilds" classes="h-screen">
							<Guilds />
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
