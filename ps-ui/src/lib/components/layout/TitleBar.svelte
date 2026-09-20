<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { osFamily } from '$lib/utils/platform';
	import { getAppState } from '$states';
	import * as m from '$i18n/messages';
	import { createNavActions } from './navActions.svelte';
	import {
		TITLE_BAR_ACTION_IDS,
		navItems,
		type NavAction,
		type NavContext,
		type NavItem
	} from './navItems';
	import { createWindowControls } from './windowControls.svelte';

	const appState = getAppState();
	const actions = createNavActions();
	const controls = createWindowControls();

	const mac = $derived(osFamily() === 'macos');
	const ctx = $derived<NavContext>({
		appState,
		desktop: PUBLIC_DESKTOP_MODE === 'true',
		expanded: true,
		titleBar: true
	});

	// The C-layout split lives here, not in navItems.ts: which side a button sits
	// on is presentation, not nav model. Together the two groups must cover
	// exactly TITLE_BAR_ACTION_IDS — a test enforces that, because anything this
	// bar fails to render the sidebar has already dropped.
	const saveActions = $derived(itemsFor(['save', 'eject']));
	const appActions = $derived(itemsFor(['open-folder', 'settings']));

	const label = $derived.by(() => {
		const file = appState.saveFile;
		if (!file) return '';
		return file.world_name ? `${file.world_name} — ${file.name}` : file.name;
	});

	function itemsFor(ids: string[]): NavItem[] {
		return ids
			.map((id) => navItems.find((item) => item.id === id))
			.filter((item): item is NavItem => Boolean(item) && (item!.visible?.(ctx) ?? true));
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

	$effect(() => controls.watch());
</script>

{#snippet action(item: NavItem)}
	<button
		class="title-bar-action"
		data-testid="title-action-{item.id}"
		title={(item.title ?? item.label)?.()}
		aria-label={(item.title ?? item.label)?.()}
		onclick={() => runAction(item.action!)}
	>
		<Icon icon={item.icon(ctx)} class="h-4 w-4 shrink-0" />
		<span class="title-bar-action-label">{item.label?.()}</span>
	</button>
{/snippet}

<div class="title-bar" class:is-native-frame={mac} data-tauri-drag-region="deep">
	{#if mac}
		<span class="title-bar-traffic-lights" data-testid="traffic-light-spacer"></span>
	{/if}

	<span class="title-bar-brand">
		<img src="/ps.png" alt="" class="h-4 w-4 shrink-0 rounded object-contain" />
		<span class="heading-gradient">PALSTUDIO</span>
	</span>

	<span class="title-bar-divider"></span>

	{#each saveActions as item (item.id)}
		{@render action(item)}
	{/each}

	<span class="title-bar-label" data-testid="title-bar-label">{label}</span>

	{#each appActions as item (item.id)}
		{@render action(item)}
	{/each}

	{#if !mac}
		<span class="title-bar-divider"></span>
		<span class="title-bar-controls">
			<button
				class="title-bar-control"
				data-testid="window-minimize"
				title={m.window_minimize()}
				aria-label={m.window_minimize()}
				onclick={() => controls.minimize()}
			>
				<Icon icon="tabler:minus" class="h-3.5 w-3.5" />
			</button>
			<button
				class="title-bar-control"
				data-testid="window-maximize"
				title={controls.maximized ? m.window_restore() : m.window_maximize()}
				aria-label={controls.maximized ? m.window_restore() : m.window_maximize()}
				onclick={() => controls.toggleMaximize()}
			>
				<Icon
					icon={controls.maximized ? 'tabler:squares' : 'tabler:square'}
					class="h-3.5 w-3.5"
				/>
			</button>
			<button
				class="title-bar-control danger"
				data-testid="window-close"
				title={m.close()}
				aria-label={m.close()}
				onclick={() => controls.close()}
			>
				<Icon icon="tabler:x" class="h-3.5 w-3.5" />
			</button>
		</span>
	{/if}
</div>
