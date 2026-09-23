<script lang="ts">
	import { ContextBar, DebugButton } from '$components/layout';
	import { PlayerList } from '$components/player';
	import { getAppState, getModalState, getPalEditorState } from '$states';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { fade } from 'svelte/transition';
	import { MessageType } from '$types';
	import { Nuke, Tooltip } from '$components/ui';
	import { send } from '$utils/websocketUtils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { EDIT_TABS, shortcutMap, toContextItems, type EditTabContext } from './editTabs';

	const { children } = $props();

	const appState = getAppState();
	const modal = getModalState();
	const palEditor = getPalEditorState();

	const shortcuts = shortcutMap();
	const visibleTabs = EDIT_TABS.filter((tab) => !tab.shortcutOnly);

	const tabContext = $derived<EditTabContext>({
		hasSave: !!appState.saveFile,
		hasPlayer: appState.selectedPlayer !== undefined,
		hasDps: !!appState.selectedPlayer?.dps,
		hasGps: appState.hasGpsAvailable
	});

	const contextItems = $derived(toContextItems(visibleTabs, tabContext));
	const activeTabId = $derived(EDIT_TABS.find((tab) => tab.href === page.url.pathname)?.id);

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

		const tabId = shortcuts[event.code];
		const tab = tabId ? EDIT_TABS.find((t) => t.id === tabId) : undefined;
		if (tab && tab.available(tabContext)) {
			event.preventDefault();
			goto(tab.href);
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

	$effect(() => {
		document.addEventListener('keydown', handleKeydown);
		return () => {
			document.removeEventListener('keydown', handleKeydown);
		};
	});
</script>

<div class="flex h-full w-full flex-col overflow-hidden">
	<!-- Tabs get their own row: sharing one with the player select pushed them off a phone. -->
	<div class="border-surface-700/40 bg-surface-900/60 flex shrink-0 flex-col md:flex-row border-b md:items-center mb-2">
		{#if appState.saveFile}
			<div class="flex min-w-0 items-center gap-2 px-2 py-1.5">
				<div class="min-w-0 flex-1">
					<PlayerList selected={appState.selectedPlayer?.uid || undefined} />
				</div>
				{#if appState.selectedPlayer && appState.settings.debug_mode}
					<DebugButton
						href={`/debug?guildId=${appState.selectedPlayer?.guild_id}&playerId=${appState.selectedPlayer!.uid}`}
					/>
				{/if}
				{#if appState.selectedPlayer}
					<Tooltip label={m.delete_entity({ entity: c.player })}>
						<button
							class="btn btn-icon hover:bg-error-500/50 shrink-0"
							onclick={handleDeletePlayer}
						>
							<Nuke size={24} />
						</button>
					</Tooltip>
				{/if}
			</div>
		{/if}
		<ContextBar id="player-tabs" items={contextItems} activeId={activeTabId ?? ''} />
	</div>
	<div class="relative flex-1 overflow-hidden">
		{#key page.url.pathname}
			<div class="absolute inset-0 overflow-y-auto" transition:fade={{ duration: 150 }}>
				{@render children()}
			</div>
		{/key}
	</div>
</div>
