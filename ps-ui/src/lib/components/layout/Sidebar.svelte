<script lang="ts">
	import { applySettings, getAppState, getModalState } from '$states';
	import Icon from '$lib/components/ui/icons/Icon.svelte';

	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { OpenFolder, SettingsModal } from '$components/modals';
	import { MessageType } from '$types';
	import { send } from '$lib/utils/websocketUtils';
	import { baseStructuresData } from '$lib/data/baseStructures.svelte';
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
	const menuIcon = $derived(menuItem.icon(ctx));
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

	// Leaving a section flushes pending edits to the backend.
	function handleNavigate(item: NavItem): void {
		if (item.id === activeTile || !appState.saveFile) return;
		appState.saveState().catch((error) => {
			console.error('Error saving state on navigate:', error);
		});
	}

	function runAction(action: NavAction): void {
		switch (action) {
			case 'toggle-expanded':
				expanded.current = !expanded.current;
				break;
			case 'save':
				appState.writeSave().catch((error) => {
					console.error('Error writing save:', error);
				});
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
		await goto('/overview');
	}

	async function handleOpenFolder(): Promise<void> {
		// @ts-ignore
		await modal.showModal(OpenFolder, {
			title: m.open_folder()
		});
	}
</script>

{#snippet actionButton(item: NavItem)}
	<button
		class="nav-link nav-link-inactive w-full text-left"
		title={(item.title ?? item.label)?.()}
		onclick={() => runAction(item.action!)}
	>
		<Icon icon={item.icon(ctx)} class="h-4 w-4 flex-shrink-0" />
		<span class="sidebar-label truncate">{item.label?.()}</span>
	</button>
{/snippet}

<aside class="sidebar flex flex-col" class:collapsed={!expanded.current}>
	<div class="sidebar-header">
		<div class="flex items-center gap-2.5 overflow-hidden">
			<img
				src="/ps.png"
				alt="PalStudio"
				class="animate-breathe h-6 w-6 shrink-0 rounded object-contain"
			/>
			<span
				class="sidebar-label heading-gradient text-xs font-extrabold tracking-tight whitespace-nowrap"
			>
				PALSTUDIO
			</span>
		</div>
		<button
			class="text-surface-500 hover:text-surface-200 transition-fast ml-auto p-1"
			title={(menuItem.title ?? menuItem.label)?.()}
			onclick={() => runAction(menuItem.action!)}
		>
			<Icon icon={menuIcon} class="h-4 w-4" />
		</button>
	</div>

	<nav class="flex-1 overflow-y-auto py-2">
		{#each navGroups as group (group.id)}
			{@const tiles = tilesForGroup(group.id)}
			{#if tiles.length > 0}
				<div class="nav-group-label">{group.label()}</div>
				{#each tiles as item (item.id)}
					{@const badge = item.badge?.(ctx) ?? null}
					<a
						href={hrefFor(item)}
						class="nav-link nav-link-{item.id === activeTile ? 'active' : 'inactive'}"
						title={(item.title ?? item.label)?.()}
						onclick={() => (item.action ? runAction(item.action) : handleNavigate(item))}
					>
						<span class="relative shrink-0">
							<Icon icon={item.icon(ctx)} class="h-4 w-4" />
							{#if badge}
								<span
									class="absolute -top-0.5 -right-0.5 h-1.5 w-1.5 rounded-full {badge === 'remote'
										? 'bg-secondary-400'
										: 'bg-success-400'}"
									title={badge === 'remote'
										? m.signal_remote_mode_chip()
										: badge === 'connected'
											? m.signal_connected_heading()
											: m.signal_nav_armed_indicator_title()}
								></span>
							{/if}
						</span>
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
