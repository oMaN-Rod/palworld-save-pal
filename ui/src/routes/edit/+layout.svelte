<script lang="ts">
	import { DebugButton } from '$components/layout';
	import { PlayerList } from '$components/player';
	import { getAppState, getModalState, getPalEditorState } from '$states';
	import { goto } from '$app/navigation';
	import { MessageType, type Player } from '$types';
	import { KeyboardShortcut, Nuke, Tooltip } from '$components/ui';
	import { send } from '$utils/websocketUtils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	const { children } = $props();

	const appState = getAppState();
	const modal = getModalState();
	const palEditor = getPalEditorState();

	const keyboardShortcuts: Record<string, string> = {
		KeyL: 'player',
		KeyT: 'technologies',
		KeyB: 'palbox',
		KeyD: 'dps',
		KeyG: 'guild',
		KeyM: 'missions',
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
			modal.isOpen ||
			palEditor.isOpen
		) {
			return;
		}

		if (event.code === 'KeyP') {
			if (appState.selectedPal) {
				event.preventDefault();
				palEditor.open();
			}
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
			title: m.delete_entity({ entity: c.player }),
			message: m.delete_entity_by_name_confirm({ name: appState.selectedPlayer?.nickname || '' }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) {
			send(MessageType.DELETE_PLAYER, {
				player_id: appState.selectedPlayer?.uid,
				origin: 'edit'
			});
			goto('/loading');
		}
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
				<PlayerList selected={appState.selectedPlayer?.uid || undefined} />
				{#if appState.selectedPlayer && appState.settings.debug_mode}
					<DebugButton
						href={`/debug?guildId=${appState.selectedPlayer?.guild_id}&playerId=${appState.selectedPlayer!.uid}`}
					/>
				{/if}
				{#if appState.selectedPlayer}
					<Tooltip label={m.delete_entity({ entity: c.player })}>
						<button class="btn ml-4 h-8 w-8 p-2 hover:bg-red-500/50" onclick={handleDeletePlayer}>
							<Nuke size={24} />
						</button>
					</Tooltip>
				{/if}
			</div>
		{:else}
			<div></div>
		{/if}
		<div id="player-tabs" class="flex gap-4">
			{#if appState.saveFile && appState.selectedPlayer}
				<KeyboardShortcut id="loadout-tab" text={m.loadout()} key="L" href="/edit/player" />
				<KeyboardShortcut id="technology-tab" text={m.technology({ count: 2 })} key="T" href="/edit/technologies" />
				<KeyboardShortcut id="palbox-tab" text={m.palbox()} key="B" href="/edit/palbox" />
			{/if}
			{#if appState.selectedPlayer?.dps}
				<KeyboardShortcut id="dps-tab" text={m.dps()} key="D" href="/edit/dps" />
			{/if}
			{#if appState.selectedPlayer}
				<KeyboardShortcut id="guild-tab" text={m.guild({ count: 1 })} key="G" href="/edit/guild" />
				<KeyboardShortcut id="missions-tab" text={m.missions()} key="M" href="/edit/missions" />
			{/if}
		</div>
		<div></div>
	</div>
	<div class="overflow-y-auto">
		{@render children()}
	</div>
</div>
