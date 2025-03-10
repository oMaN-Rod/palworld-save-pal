<script lang="ts">
	import PalEdit from './components/pal/Edit.svelte';
	import PlayerEdit from './components/PlayerEdit.svelte';
	import PalBox from './components/palbox/PalBox.svelte';
	import { DebugButton, PlayerList } from '$components';
	import {
		getAppState,
		getModalState,
		getNavigationState,
		getSocketState,
		type Tab
	} from '$states';
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import { goto } from '$app/navigation';
	import Guilds from './components/guilds/Guilds.svelte';
	import Technologies from './components/technologies/Technologies.svelte';
	import { MessageType, type Player } from '$types';
	import { Trash } from 'lucide-svelte';
	import { Tooltip } from '$components/ui';

	const appState = getAppState();
	const nav = getNavigationState();
	const ws = getSocketState();
	const modal = getModalState();

	$effect(() => {
		if (!appState.saveFile) {
			goto('/file');
		}
	});

	async function handleDeletePlayer() {
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Player',
			message: 'Are you sure you want to delete this player? This action cannot be undone.',
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (confirmed) {
			const message = {
				type: MessageType.DELETE_PLAYER,
				data: {
					player_id: appState.selectedPlayer?.uid,
					origin: 'edit'
				}
			};
			ws.send(JSON.stringify(message));
			goto('/loading');
		}
	}
</script>

<div class="flex h-full w-full overflow-hidden">
	<div class="relative h-full w-full">
		{#if appState.saveFile}
			<div
				class="absolute left-2 right-2 top-0 flex min-w-72 flex-row items-center justify-between"
			>
				<div class="flex items-center">
					<PlayerList
						selected={appState.selectedPlayer?.uid || undefined}
						onselect={(player: Player) => (appState.selectedPlayer = player)}
					/>
					{#if appState.selectedPlayer && appState.settings.debug_mode}
						<DebugButton
							href={`/debug?guildId=${appState.selectedPlayer?.guild_id}&playerId=${appState.selectedPlayer!.uid}`}
						/>
					{/if}
					{#if appState.selectedPlayer}
						<Tooltip label="Delete player">
							<button class="btn p-2 hover:bg-red-500/50" onclick={handleDeletePlayer}>
								<Trash />
							</button>
						</Tooltip>
					{/if}
				</div>
				<a
					href="https://discord.gg/YWZFPy9G8J"
					target="_blank"
					rel="noopener noreferrer"
					class="mr-2 inline-flex items-center rounded-md bg-indigo-600 px-3 py-2 text-sm font-medium text-white hover:bg-indigo-700"
				>
					<svg
						class="mr-2 h-5 w-5"
						fill="currentColor"
						viewBox="0 0 24 24"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515a.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0a12.64 12.64 0 0 0-.617-1.25a.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057a19.9 19.9 0 0 0 5.993 3.03a.078.078 0 0 0 .084-.028a14.09 14.09 0 0 0 1.226-1.994a.076.076 0 0 0-.041-.106a13.107 13.107 0 0 1-1.872-.892a.077.077 0 0 1-.008-.128a10.2 10.2 0 0 0 .372-.292a.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127a12.299 12.299 0 0 1-1.873.892a.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028a19.839 19.839 0 0 0 6.002-3.03a.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.956-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.955-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.946 2.418-2.157 2.418z"
						/>
					</svg>
					Support
				</a>
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
