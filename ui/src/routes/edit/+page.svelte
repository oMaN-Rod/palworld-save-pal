<script lang="ts">
	import PalEdit from './components/pal/Edit.svelte';
	import PlayerEdit from './components/PlayerEdit.svelte';
	import PalBox from './components/palbox/PalBox.svelte';
	import { DebugButton, PlayerList } from '$components';
	import { getAppState, getNavigationState, type Tab } from '$states';
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import { goto } from '$app/navigation';
	import Guilds from './components/guilds/Guilds.svelte';
	import Technologies from './components/technologies/Technologies.svelte';
	import type { Player } from '$types';
	import { Bug } from 'lucide-svelte';

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
			<div class="absolute left-2 top-0 flex min-w-72 shrink-0 flex-row items-center">
				<PlayerList
					selected={appState.selectedPlayer?.uid || undefined}
					onselect={(player: Player) => (appState.selectedPlayer = player)}
				/>
				{#if appState.selectedPlayer && appState.settings.debug_mode}
					<DebugButton
						href={`/debug?guildId=${appState.selectedPlayer?.guild_id}&playerId=${appState.selectedPlayer!.uid}`}
					/>
				{/if}
			</div>
			<Tabs
				listJustify="justify-center"
				value={nav.activeTab}
				classes="flex h-full flex-col mt-4"
				onValueChange={(e) => {
					nav.activeTab = e.value as Tab;
				}}
			>
				{#snippet list()}
					<div class="shrink-0">
						<Tabs.Control value="player">Player</Tabs.Control>
						<Tabs.Control value="technologies">Technologies</Tabs.Control>
						<Tabs.Control value="pal-box">Pal Box</Tabs.Control>
						<Tabs.Control value="guilds">Guild</Tabs.Control>
						<Tabs.Control value="pal">Pal</Tabs.Control>
					</div>
				{/snippet}
				{#snippet content()}
					<div class="grow overflow-hidden">
						<Tabs.Panel value="player" classes="h-screen">
							<PlayerEdit />
						</Tabs.Panel>
						<Tabs.Panel value="technologies" classes="h-screen">
							<Technologies />
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
