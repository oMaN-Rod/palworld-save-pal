<script lang="ts">
	import { applySettings, getAppState, getModalState } from '$states';

	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { OpenFolder, SettingsModal } from '$components/modals';
	import { MessageType } from '$types';
	import { send } from '$lib/utils/websocketUtils';
	import { baseStructuresData } from '$lib/data';
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

	const activeTile = $derived(activeNavId(page.url.pathname, ctx));
	const menuItem = $derived(navItems.find((item) => item.id === 'menu')!);
	const MenuIcon = $derived(menuItem.icon(ctx));
	const actionItems = $derived([
		...itemsFor('header').filter((item) => item.id !== 'menu'),
		...itemsFor('footer')
	]);

	function itemsFor(section: 'header' | 'footer'): NavItem[] {
		return navItems.filter((item) => item.section === section && (item.visible?.(ctx) ?? true));
	}

	function tilesForGroup(group: NavGroup): NavItem[] {
		return navItems.filter(
			(item) => item.section === 'tiles' && item.group === group && (item.visible?.(ctx) ?? true)
		);
	}

	function hrefFor(item: NavItem): string | undefined {
		return typeof item.href === 'string' ? item.href : item.href?.(ctx);
	}

	// Leaving a section flushes pending edits to the backend, matching the
	// behaviour the Skeleton rail drove from its active-tile change.
	function handleNavigate(item: NavItem): void {
		if (item.id === activeTile || !appState.saveFile) return;
		appState.saveState();
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

	async function handleLanguageSelect(): Promise<void> {
		// @ts-ignore
		const result = await modal.showModal<string>(SettingsModal, {
			title: m.settings(),
			settings: appState.settings
		});

		if (result) {
			applySettings();
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
		baseStructuresData.reset();
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

{#snippet actionButton(item: NavItem)}
	{@const Icon = item.icon(ctx)}
	<button
		class="nav-link nav-link-inactive w-full text-left"
		title={(item.title ?? item.label)?.()}
		onclick={() => runAction(item.action!)}
	>
		<Icon class="h-4 w-4 flex-shrink-0" />
		<span class="sidebar-label truncate">{item.label?.()}</span>
	</button>
{/snippet}

<aside class="sidebar flex flex-col" class:collapsed={!expanded.current}>
	<div class="sidebar-header">
		<div class="flex items-center gap-2.5 overflow-hidden">
			<img
				src="/psp.png"
				alt="PSP"
				class="animate-breathe h-6 w-6 shrink-0 rounded object-contain"
			/>
			<span
				class="sidebar-label heading-gradient text-xs font-extrabold tracking-tight whitespace-nowrap"
			>
				PALWORLD SAVE PAL
			</span>
		</div>
		<button
			class="text-surface-500 hover:text-surface-200 transition-fast ml-auto p-1"
			title={(menuItem.title ?? menuItem.label)?.()}
			onclick={() => runAction(menuItem.action!)}
		>
			<MenuIcon class="h-4 w-4" />
		</button>
	</div>

	<nav class="flex-1 overflow-y-auto py-2">
		{#each navGroups as group (group.id)}
			{@const tiles = tilesForGroup(group.id)}
			{#if tiles.length > 0}
				<div class="nav-group-label">{group.label()}</div>
				{#each tiles as item (item.id)}
					{@const Icon = item.icon(ctx)}
					<a
						href={hrefFor(item)}
						class="nav-link nav-link-{item.id === activeTile ? 'active' : 'inactive'}"
						title={(item.title ?? item.label)?.()}
						onclick={() => (item.action ? runAction(item.action) : handleNavigate(item))}
					>
						<Icon class="h-4 w-4 shrink-0" />
						<span class="sidebar-label truncate">{item.label?.()}</span>
					</a>
				{/each}
			{/if}
		{/each}
	</nav>

	<div class="border-surface-700/30 border-t py-2">
		{#each actionItems as item (item.id)}
			<div class="flex justify-center">
				{@render actionButton(item)}
			</div>
		{/each}
	</div>
</aside>
