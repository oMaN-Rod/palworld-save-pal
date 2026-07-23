<script lang="ts">
	import { getAppState, getModalState } from '$states';

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
		navGroups,
		type NavAction,
		type NavContext,
		type NavItem,
		type NavGroup
	} from './navItems';

	let appState = getAppState();
	let modal = getModalState();
	let expanded = persistedState('navbar.expanded', false);

	const desktop = PUBLIC_DESKTOP_MODE === 'true';
	const ctx = $derived<NavContext>({ appState, desktop, expanded: expanded.current });

	function itemsFor(section: 'header' | 'footer'): NavItem[] {
		return navItems.filter((item) => item.section === section && (item.visible?.(ctx) ?? true));
	}

	function tilesForGroup(group: NavGroup): NavItem[] {
		return navItems.filter(
			(item) => item.section === 'tiles' && item.group === group && (item.visible?.(ctx) ?? true)
		);
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

<aside class="sidebar flex flex-col" class:collapsed={!expanded.current}>
	<!-- Header: logo + title + collapse toggle -->
	<div class="sidebar-header">
		<div class="flex items-center gap-2.5 overflow-hidden">
			<img
				src="/psp.png"
				alt="PSP"
				class="h-7 w-7 flex-shrink-0 rounded object-contain animate-breathe"
			/>
			{#if expanded.current}
				<span class="sidebar-label heading-gradient text-lg font-extrabold tracking-tight whitespace-nowrap">
					Palworld Save Pals
				</span>
			{/if}
		</div>
		{#if expanded.current}
			<button
				class="ml-auto text-surface-500 hover:text-surface-200 transition-fast p-1"
				title={m.toggle_entity({ entity: '' })}
				onclick={() => runAction('toggle-expanded')}
			>
				<!-- icon resolved inline to avoid dynamic-component overhead for a single chevron -->
				<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6" /></svg>
			</button>
		{:else}
			<button
				class="mx-auto text-surface-500 hover:text-surface-200 transition-fast p-1"
				title={m.toggle_entity({ entity: '' })}
				onclick={() => runAction('toggle-expanded')}
			>
				<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6" /></svg>
			</button>
		{/if}
	</div>

	<!-- Body: grouped nav items -->
	<nav class="flex-1 overflow-y-auto py-2">
		{#each navGroups as group (group.id)}
			{@const tiles = tilesForGroup(group.id)}
			{#if tiles.length > 0}
				{#if expanded.current}
					<div class="nav-group-label">{group.label}</div>
				{/if}
				{#each tiles as item (item.id)}
					{@const Icon = item.icon(ctx)}
					{@const isActive = item.id === activeTile}
						{@const needsSave = item.href && item.href !== '/' && item.href !== '/file' && item.href !== '/upload' && item.href !== '/about' && item.href !== '/docs'}
					<a
						href={item.href}
						class="nav-link nav-link-{isActive ? 'active' : 'inactive'}"
						class:nav-link-disabled={needsSave && !appState.saveFile}
						title={(item.title ?? item.label)?.()}
						onclick={item.action ? () => runAction(item.action!) : undefined}
					>
						<Icon class="h-4 w-4 flex-shrink-0" />
						{#if expanded.current}
							<span class="sidebar-label truncate">{item.label?.()}</span>
						{/if}
					</a>
				{/each}
			{/if}
		{/each}
	</nav>

	<!-- Footer: action items (save/eject from header section + open-folder/settings) -->
	<div class="border-t border-surface-700/30 py-2 flex-shrink-0">
		{#if expanded.current}
			<!-- save + eject action buttons (header section) -->
			{#each itemsFor('header').filter((i) => i.id !== 'menu') as item (item.id)}
				{@const Icon = item.icon(ctx)}
				<button
					class="nav-link nav-link-inactive w-full text-left"
					title={(item.title ?? item.label)?.()}
					onclick={() => runAction(item.action!)}
				>
					<Icon class="h-4 w-4 flex-shrink-0" />
					<span class="sidebar-label truncate">{item.label?.()}</span>
				</button>
			{/each}
			<!-- footer actions (open-folder/settings) -->
			{#each itemsFor('footer') as item (item.id)}
				{@const Icon = item.icon(ctx)}
				<button
					class="nav-link nav-link-inactive w-full text-left"
					title={(item.title ?? item.label)?.()}
					onclick={() => runAction(item.action!)}
				>
					<Icon class="h-4 w-4 flex-shrink-0" />
					<span class="sidebar-label truncate">{item.label?.()}</span>
				</button>
			{/each}
		{:else}
			<!-- collapsed: icon-only -->
			{#each [...itemsFor('header').filter((i) => i.id !== 'menu'), ...itemsFor('footer')] as item (item.id)}
				{@const Icon = item.icon(ctx)}
				<button
					class="nav-link nav-link-inactive mx-auto"
					title={(item.title ?? item.label)?.()}
					onclick={() => runAction(item.action!)}
				>
					<Icon class="h-4 w-4 flex-shrink-0" />
				</button>
			{/each}
		{/if}
	</div>
</aside>
