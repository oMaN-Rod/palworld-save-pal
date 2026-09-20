<script lang="ts">
	import { fly, fade } from 'svelte/transition';
	import { getAppState } from '$states';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { trapFocus } from '$lib/utils/focusTrap';

	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { page } from '$app/state';
	import * as m from '$i18n/messages';
	import { desktopChrome } from '$lib/utils/platform';
	import { createNavActions } from './navActions.svelte';
	import {
		activeNavId,
		navItems,
		navGroups,
		isTitleBarAction,
		type NavAction,
		type NavContext,
		type NavItem,
		type NavGroup
	} from './navItems';

	let { open = $bindable(false), onClose }: { open?: boolean; onClose: () => void } = $props();

	let appState = getAppState();
	const actions = createNavActions();

	const desktop = PUBLIC_DESKTOP_MODE === 'true';
	const ctx = $derived<NavContext>({
		appState,
		desktop,
		expanded: true,
		titleBar: desktopChrome()
	});

	const activeTile = $derived(activeNavId(page.url.pathname, ctx));

	function tilesForGroup(group: NavGroup): NavItem[] {
		return navItems.filter(
			(item) => item.section === 'tiles' && item.group === group && (item.visible?.(ctx) ?? true)
		);
	}

	function itemsFor(section: 'header' | 'footer'): NavItem[] {
		return navItems.filter(
			(item) =>
				item.section === section &&
				(item.visible?.(ctx) ?? true) &&
				!(ctx.titleBar && isTitleBarAction(item.id))
		);
	}

	const actionItems = $derived([
		...itemsFor('header').filter((item) => item.id !== 'menu'),
		...itemsFor('footer')
	]);

	function hrefFor(item: NavItem): string | undefined {
		return typeof item.href === 'string' ? item.href : item.href?.(ctx);
	}

	function handleNavigate(item: NavItem): void {
		if (item.id !== activeTile && appState.saveFile) {
			appState.saveState().catch((error) => {
				console.error('Error saving state on navigate:', error);
			});
		}
		onClose();
	}

	function runAction(action: NavAction): void {
		switch (action) {
			case 'save':
				actions.save();
				break;
			case 'eject':
				void actions.eject();
				break;
			case 'open-folder':
				void actions.openFolder();
				break;
			case 'settings':
				void actions.settings();
				break;
		}
	}

	let panel: HTMLElement | null = $state(null);

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key !== 'Escape') return;
		event.preventDefault();
		onClose();
	}

	$effect(() => {
		if (!open || !panel) return;
		document.addEventListener('keydown', handleKeydown);
		const releaseFocus = trapFocus(panel);
		return () => {
			document.removeEventListener('keydown', handleKeydown);
			releaseFocus();
		};
	});
</script>

{#if open}
	<div
		class="fixed inset-0 z-[45000] bg-black/60"
		data-testid="nav-drawer-backdrop"
		onclick={onClose}
		role="presentation"
		transition:fade={{ duration: 150 }}
	></div>

	<div
		bind:this={panel}
		class="nav-drawer fixed inset-y-0 left-0 z-[45001] flex flex-col shadow-2xl"
		style:padding-top="env(safe-area-inset-top)"
		style:padding-bottom="env(safe-area-inset-bottom)"
		role="dialog"
		aria-modal="true"
		aria-label={m.navigation()}
		transition:fly={{ x: -300, duration: 220 }}
	>
		<div class="flex items-center gap-2 p-3">
			<span class="flex-1 text-base font-extrabold">PalStudio</span>
			<button
				type="button"
				class="flex size-11 items-center justify-center rounded-lg"
				aria-label={m.close()}
				onclick={onClose}
			>
				<Icon icon="tabler:x" class="size-5" />
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
							onclick={() => handleNavigate(item)}
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

		{#if actionItems.length > 0}
			<div class="border-surface-700/30 border-t py-2">
				{#each actionItems as item (item.id)}
					<button
						class="nav-link nav-link-inactive w-full text-left"
						data-testid="nav-action-{item.id}"
						title={(item.title ?? item.label)?.()}
						aria-label={(item.title ?? item.label)?.()}
						onclick={() => runAction(item.action!)}
					>
						<Icon icon={item.icon(ctx)} class="h-4 w-4 flex-shrink-0" />
						<span class="sidebar-label truncate">{item.label?.()}</span>
					</button>
				{/each}
			</div>
		{/if}
	</div>
{/if}
