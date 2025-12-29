<script lang="ts">
	import { DebugButton, PlayerList } from '$components';
	import { getAppState, getModalState } from '$states';
	import { goto } from '$app/navigation';
	import { MessageType, type Player } from '$types';
	import { KeyboardShortcut, Tooltip } from '$components/ui';
	import { send } from '$utils/websocketUtils';
	import Nuke from '$components/ui/icons/Nuke.svelte';

	const { children } = $props();

	const appState = getAppState();
	const modal = getModalState();

	const keyboardShortcuts: Record<string, string> = {
		KeyL: 'player',
		KeyT: 'technologies',
		KeyB: 'palbox',
		KeyD: 'dps',
		KeyG: 'guild',
		KeyM: 'missions',
		KeyP: 'pal',
		KeyS: 'gps'
	};

	function isTabAvailable(tab: string): boolean {
		switch (tab) {
			case 'dps':
				return !!appState.selectedPlayer?.dps;
			case 'gps':
				return !!appState.gps;
			default:
				return appState.selectedPlayer !== undefined;
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (
			event.target instanceof HTMLInputElement ||
			event.target instanceof HTMLTextAreaElement ||
			event.ctrlKey ||
			event.altKey ||
			event.metaKey ||
			event.shiftKey ||
			modal.isOpen
		) {
			return;
		}

		const shortcutTab = keyboardShortcuts[event.code];
		if (shortcutTab && isTabAvailable(shortcutTab)) {
			event.preventDefault();
			goto(`/edit/${shortcutTab}`);
		}
	}

	async function handleDeletePlayer() {
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Player',
			message: 'Are you sure you want to delete this player? This action cannot be undone.',
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (confirmed) {
			send(MessageType.DELETE_PLAYER, {
				player_id: appState.selectedPlayer?.uid,
				origin: 'edit'
			});
			goto('/loading');
		}
	}

	async function handleOnSelectPlayer(player: Player) {
		appState.selectedPlayer = player;
		goto('/edit/player');
	}

	// Add keyboard event listener when component mounts
	$effect(() => {
		document.addEventListener('keydown', handleKeydown);
		return () => {
			document.removeEventListener('keydown', handleKeydown);
		};
	});
</script>

<div class="flex h-full w-full flex-col overflow-hidden">
	<div class="mx-2 flex min-w-72 items-center justify-between">
		{#if appState.saveFile}
			<div class="flex items-center">
				<PlayerList
					selected={appState.selectedPlayer?.uid || undefined}
					onselect={(player: Player) => handleOnSelectPlayer(player)}
				/>
				{#if appState.selectedPlayer && appState.settings.debug_mode}
					<DebugButton
						href={`/debug?guildId=${appState.selectedPlayer?.guild_id}&playerId=${appState.selectedPlayer!.uid}`}
					/>
				{/if}
				{#if appState.selectedPlayer}
					<Tooltip label="Delete player">
						<button class="btn ml-4 h-8 w-8 p-2 hover:bg-red-500/50" onclick={handleDeletePlayer}>
							<Nuke size={24} />
						</button>
					</Tooltip>
				{/if}
			</div>
		{:else}
			<div></div>
		{/if}
		<div class="flex gap-4">
			{#if appState.saveFile && appState.selectedPlayer}
				<KeyboardShortcut text="Loadout" key="L" href="/edit/player" />
				<KeyboardShortcut text="Technologies" key="T" href="/edit/technologies" />
				<KeyboardShortcut text="Pal Box" key="B" href="/edit/palbox" />
			{/if}
			{#if appState.selectedPlayer?.dps}
				<KeyboardShortcut text="DPS" key="D" href="/edit/dps" />
			{/if}
			{#if appState.selectedPlayer}
				<KeyboardShortcut text="Guild" key="G" href="/edit/guild" />
				<KeyboardShortcut text="Missions" key="M" href="/edit/missions" />
			{/if}
			{#if appState.selectedPal}
				<KeyboardShortcut text="Pal" key="P" href="/edit/pal" />
			{/if}
		</div>
		<a
			href="https://discord.gg/YWZFPy9G8J"
			target="_blank"
			rel="noopener noreferrer"
			class="mr-2 inline-flex rounded-md bg-indigo-600 px-3 py-2 text-sm font-medium text-white hover:bg-indigo-700 {appState.saveFile
				? ''
				: 'mt-3'}"
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
	<div class="overflow-y-auto">
		{@render children()}
	</div>
</div>
