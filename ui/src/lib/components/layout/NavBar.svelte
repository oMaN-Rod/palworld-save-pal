<script lang="ts">
	import { getAppState, getModalState } from '$states';
	import { Navigation } from '@skeletonlabs/skeleton-svelte';

	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { OpenFolder, SettingsModal } from '$components/modals';
	import { MessageType } from '$types';
	import { send } from '$lib/utils/websocketUtils';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import * as m from '$i18n/messages';
	import { persistedState } from 'svelte-persisted-state';
	import { getStoredSessionId, clearSessionPersistence } from '$lib/utils/sessionPersistence';
	import {
		activeNavId,
		navItems,
		type NavAction,
		type NavContext,
		type NavItem,
		type NavSection
	} from './navItems';

	let appState = getAppState();
	let modal = getModalState();
	let expanded = persistedState('navbar.expanded', false);

	const desktop = PUBLIC_DESKTOP_MODE === 'true';
	const ctx = $derived<NavContext>({ appState, desktop, expanded: expanded.current });

	function itemsFor(section: NavSection): NavItem[] {
		return navItems.filter((item) => item.section === section && (item.visible?.(ctx) ?? true));
	}

	function runAction(action: NavAction): void {
		switch (action) {
			case 'toggle-expanded':
				expanded.current = !expanded.current;
				break;
			case 'save':
				appState.writeSave();
				break;
			case 'eject':
				handleEject();
				break;
			case 'open-folder':
				handleOpenFolder();
				break;
			case 'settings':
				handleLanguageSelect();
				break;
		}
	}

	const activeTile = $derived(activeNavId(page.url.pathname));

	async function handleLanguageSelect(): Promise<void> {
		// @ts-ignore
		const result = await modal.showModal<string>(SettingsModal, {
			title: m.settings(),
			settings: appState.settings
		});

		if (result) {
			send(MessageType.UPDATE_SETTINGS, { ...appState.settings });
			setTimeout(() => {
				location.reload();
			}, 500);
		}
	}

	async function handleEject(): Promise<void> {
		const sessionId = getStoredSessionId();
		if (sessionId) {
			send(MessageType.EJECT_SESSION, { session_id: sessionId });
		}
		appState.resetState();
		clearSessionPersistence();
		await goto('/file');
	}

	async function handleOpenFolder(): Promise<void> {
		// @ts-ignore
		await modal.showModal(OpenFolder, {
			title: m.open_folder()
		});
	}
</script>

{#snippet tile(item: NavItem)}
	{@const Icon = item.icon(ctx)}
	<Navigation.Tile
		id={item.id}
		labelExpanded={item.label?.()}
		expandedClasses="text-xs 2xl:text-base"
		title={(item.title ?? item.label)?.()}
		href={item.href}
		onclick={item.action ? () => runAction(item.action!) : undefined}
		active={item.href ? 'active-nav-tile' : undefined}
	>
		<Icon class="h-4 w-4 2xl:h-6 2xl:w-6"/>
	</Navigation.Tile>
{/snippet}

<Navigation.Rail
	width="48px"
	widthExpanded="w-auto"
	value={activeTile}
	onValueChange={() => appState.saveState()}
	expanded={expanded.current}
	background="bg-surface-900"
	classes="nav-rail"
>
	{#snippet header()}
		{#each itemsFor('header') as item (item.id)}
			{@render tile(item)}
		{/each}
	{/snippet}
	{#snippet tiles()}
		{#each itemsFor('tiles') as item (item.id)}
			{@render tile(item)}
		{/each}
	{/snippet}
	{#snippet footer()}
		{#each itemsFor('footer') as item (item.id)}
			{@render tile(item)}
		{/each}
	{/snippet}
</Navigation.Rail>
