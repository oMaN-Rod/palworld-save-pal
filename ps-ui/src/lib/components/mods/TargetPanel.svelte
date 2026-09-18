<script module lang="ts">
	export type ModsTab =
		| 'mods'
		| 'order'
		| 'frameworks'
		| 'conflicts'
		| 'live'
		| 'worlds'
		| 'discover'
		| 'scan'
		| 'backups';
</script>

<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { untrack } from 'svelte';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { Button, Card, Popover } from '$components/ui';
	import { getModalState, getModsState, getServerState } from '$states';
	import { cn } from '$theme';
	import { MessageType } from '$types';
	import type { TargetLayoutJson } from '$types';
	import * as m from '$i18n/messages';
	import { confirmRemoveTarget, hazardText, targetHazards, targetName } from './targets';
	import { frameworkErrorText } from './frameworkText';
	import ModList from './ModList.svelte';
	import ActionMenu, { menuItemClass } from './ActionMenu.svelte';
	import ApplyBar from './ApplyBar.svelte';
	import ApplyStatus from './ApplyStatus.svelte';
	import ProfileBar from './ProfileBar.svelte';
	import LoadOrderPanel from './LoadOrderPanel.svelte';
	import ScanPanel from './ScanPanel.svelte';
	import BackupsPanel from './BackupsPanel.svelte';
	import WorldsPanel from './WorldsPanel.svelte';
	import LaunchStatus from './LaunchStatus.svelte';
	import FrameworksPanel from './FrameworksPanel.svelte';
	import ConflictsPanel from './ConflictsPanel.svelte';
	import LivePanel from './LivePanel.svelte';
	import DiscoverPanel from './DiscoverPanel.svelte';
	import { canLaunch, canLinkWorlds } from './modCapabilities';

	let { targetId, tab = $bindable('mods') }: { targetId: string; tab?: ModsTab } = $props();

	const modsState = getModsState();
	const modal = getModalState();
	const serverState = getServerState();

	const target = $derived(modsState.targets.find((entry) => entry.id === targetId));
	const name = $derived(target ? targetName(target, serverState.servers) : '');
	const hazards = $derived(target ? targetHazards(target) : []);
	const missing = $derived(target === undefined);
	const subscribedCount = $derived(modsState.subscribedCandidates(targetId).length);
	const palModSettingsUnreadable = $derived(
		modsState.scanWarnings[targetId]?.includes('palmodsettings_unreadable') ?? false
	);
	const scanError = $derived(modsState.lastErrorFor(MessageType.MOD_TARGET_SCAN, targetId));
	const movableHazards = new Set(['workshop_proxy_dll', 'amity_legacy_folder']);
	const hazardOutcome = $derived(modsState.lastHazardRemove[targetId]);
	const hazardError = $derived(
		modsState.lastErrorFor(MessageType.FRAMEWORK_HAZARD_REMOVE, targetId)
	);
	const tabsId = $props.id();

	let requestedTargets = $state.raw<unknown>();
	const noTarget = $derived(
		missing &&
			modsState.targetsLoaded &&
			requestedTargets !== undefined &&
			modsState.targets !== requestedTargets
	);

	const layoutLabels: [keyof TargetLayoutJson, () => string][] = [
		['ue4ss_mods_dir', m.mods_layout_ue4ss_mods_dir],
		['mods_txt', m.mods_layout_mods_txt],
		['palschema_mods_dir', m.mods_layout_palschema_mods_dir],
		['paks_mods_dir', m.mods_layout_paks_mods_dir],
		['logicmods_dir', m.mods_layout_logicmods_dir],
		['nativemods_dir', m.mods_layout_nativemods_dir],
		['workshop_local_dir', m.mods_layout_workshop_local_dir],
		['palmodsettings_ini', m.mods_layout_palmodsettings_ini]
	];

	const layoutRows = $derived.by(() => {
		const layout = target?.layout;
		if (!layout) return [];
		return layoutLabels.flatMap(([key, label]) => {
			const dir = layout[key];
			return dir ? [{ key, label: label(), dir }] : [];
		});
	});

	const allTabs: { id: ModsTab; label: () => string; icon: string }[] = [
		{ id: 'mods', label: m.mods_panel_title, icon: 'tabler:package' },
		{ id: 'order', label: m.mods_tab_order, icon: 'tabler:arrows-sort' },
		{ id: 'frameworks', label: m.mods_tab_frameworks, icon: 'tabler:puzzle' },
		{ id: 'conflicts', label: m.mods_tab_conflicts, icon: 'tabler:git-merge' },
		{ id: 'live', label: m.mods_tab_live, icon: 'tabler:heartbeat' },
		{ id: 'worlds', label: m.mods_tab_worlds, icon: 'tabler:world' },
		{ id: 'discover', label: m.mods_tab_discover, icon: 'tabler:world-download' },
		{ id: 'scan', label: m.mods_tab_scan, icon: 'tabler:search' },
		{ id: 'backups', label: m.mods_tab_backups, icon: 'tabler:archive' }
	];

	const remoteMode = getRemoteMode();
	const mode = $derived({ desktop: PUBLIC_DESKTOP_MODE === 'true', remote: remoteMode.active });
	const launchable = $derived(target ? canLaunch(target, mode) : false);
	const worldsShown = $derived(target ? canLinkWorlds(target, mode) : false);
	const liveShown = $derived(target?.kind === 'client');
	const discoverShown = $derived(mode.desktop && !mode.remote);
	const tabs = $derived(
		allTabs.filter(
			(entry) =>
				(entry.id !== 'worlds' || worldsShown) &&
				(entry.id !== 'live' || liveShown) &&
				(entry.id !== 'discover' || discoverShown)
		)
	);

	$effect(() => {
		if (tab === 'worlds' && target && !worldsShown) untrack(() => (tab = 'mods'));
	});

	$effect(() => {
		if (tab === 'live' && target && !liveShown) untrack(() => (tab = 'mods'));
	});

	$effect(() => {
		if (tab === 'discover' && !discoverShown) untrack(() => (tab = 'mods'));
	});

	function showTab(next: ModsTab) {
		tab = next;
	}

	function moveTab(event: KeyboardEvent) {
		const index = tabs.findIndex((entry) => entry.id === tab);
		const targets: Record<string, number> = {
			ArrowRight: (index + 1) % tabs.length,
			ArrowLeft: (index - 1 + tabs.length) % tabs.length,
			Home: 0,
			End: tabs.length - 1
		};
		const next = tabs[targets[event.key]];
		if (!next) return;
		event.preventDefault();
		showTab(next.id);
		document.getElementById(`${tabsId}-${next.id}`)?.focus();
	}

	let loaded: { id: string; resets: number } | undefined;

	/** A reload after a reset waits for the target list to list this target again. */
	$effect(() => {
		const id = targetId;
		const resets = modsState.resets;
		const present = !missing;
		untrack(() => {
			const reload = loaded?.id === id;
			if (reload && (loaded?.resets === resets || !present)) return;
			loaded = { id, resets };
			modsState.loadProfiles(id);
			modsState.plan(id);
			if (reload) modsState.refreshScan(id);
			else modsState.scanOnce(id);
		});
	});

	$effect(() => {
		if (!missing) return;
		void modsState.resets;
		untrack(() => {
			requestedTargets = modsState.targets;
			modsState.loadTargets();
			modsState.loadLibrary();
		});
	});
</script>

{#if target}
	<div class="flex h-full flex-col gap-4">
		<div class="flex flex-wrap items-start justify-between gap-x-4 gap-y-3">
			<div class="flex min-w-0 flex-col gap-2">
				<div class="min-w-0">
					<h2 class="flex min-w-0 items-center gap-2 text-xl font-bold">
						<span class="truncate">{name}</span>
						<span
							class={[
								'shrink-0 rounded-sm px-1.5 py-0.5 text-[10px] font-medium uppercase',
								target.kind === 'server'
									? 'bg-cyan-500/15 text-cyan-400'
									: 'bg-blue-500/15 text-blue-400'
							]}
						>
							{target.kind === 'server' ? m.mods_target_kind_server() : m.mods_target_kind_client()}
						</span>
					</h2>
					<p class="text-surface-400 truncate font-mono text-xs" title={target.root_path}>
						{target.root_path}
					</p>
				</div>
				<ProfileBar {targetId} />
			</div>
			<div class="flex shrink-0 items-center gap-2">
				<ApplyStatus {targetId} onShowTab={showTab} />
				{#if layoutRows.length > 0}
					<Popover position="bottom-end" popoverClass="max-w-xl">
						<Button
							variant="ghost"
							size="sm"
							aria-label={m.mods_where_mods_go()}
							title={m.mods_where_mods_go()}
						>
							<Icon icon="tabler:folder" size={16} />
						</Button>
						{#snippet content()}
							<h4 class="text-surface-300 text-sm font-medium">{m.mods_where_mods_go()}</h4>
							<dl class="mt-2 grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-1 text-sm">
								{#each layoutRows as row (row.key)}
									<dt class="text-surface-400">{row.label}</dt>
									<dd class="truncate font-mono text-xs leading-5" title={row.dir}>{row.dir}</dd>
								{/each}
							</dl>
						{/snippet}
					</Popover>
				{/if}
				{#if launchable}
					<Button
						size="sm"
						variant="primary"
						class="flex items-center gap-2"
						loading={modsState.launching[targetId] ?? false}
						onclick={() => modsState.launch(targetId)}
					>
						{#if !modsState.launching[targetId]}
							<Icon icon="tabler:player-play" size={14} />
						{/if}
						{m.mods_launch()}
					</Button>
				{/if}
				{#if target.kind === 'client'}
					<ActionMenu label={m.mods_target_actions()}>
						{#snippet items({ close })}
							<button
								role="menuitem"
								class={cn(menuItemClass, 'text-red-400')}
								onclick={() => {
									close();
									confirmRemoveTarget(target, name, modal, modsState);
								}}
							>
								<Icon icon="tabler:trash-x" size={14} />
								{m.mods_target_remove()}
							</button>
						{/snippet}
					</ActionMenu>
				{/if}
			</div>
		</div>

		{#if hazards.length > 0 || hazardOutcome || hazardError}
			<Card padding="p-3" class="border-warning-500/40 bg-warning-500/10 border">
				<h4 class="text-warning-400 mb-1 flex items-center gap-2 text-xs font-medium uppercase">
					<Icon icon="tabler:alert-triangle" size={14} />
					{m.mods_hazards_title()}
				</h4>
				<ul class="flex flex-col gap-2 text-sm">
					{#each hazards as hazard (hazard)}
						<li class="flex flex-wrap items-center justify-between gap-2">
							<span>{hazardText(hazard)}</span>
							{#if movableHazards.has(hazard)}
								<Button
									size="sm"
									variant="ghost"
									disabled={modsState.frameworkBusy[targetId] ?? false}
									onclick={() => modsState.removeHazard(targetId, hazard)}
								>
									{m.mods_hazard_move_named({ hazard: hazardText(hazard) })}
								</Button>
							{/if}
						</li>
					{/each}
				</ul>
				{#if hazardOutcome}
					<p class="text-success-400 mt-2 text-xs" role="status">
						{m.mods_hazard_moved({
							count: hazardOutcome.moved.length,
							dir: hazardOutcome.backup_dir
						})}
					</p>
				{/if}
				{#if hazardError}
					<p class="text-error-400 mt-2 text-xs" role="alert">{frameworkErrorText(hazardError)}</p>
				{/if}
			</Card>
		{/if}

		<LaunchStatus {targetId} />

		{#if palModSettingsUnreadable && tab !== 'scan'}
			<p class="text-warning-400 flex items-center gap-2 text-xs">
				<Icon icon="tabler:alert-triangle" size={12} class="shrink-0" />
				<span>{m.mods_panel_palmodsettings_unreadable()}</span>
			</p>
		{/if}

		{#if scanError && tab !== 'scan'}
			<p class="text-error-400 flex items-center gap-2 text-xs" role="alert">
				<Icon icon="tabler:alert-circle" size={12} class="shrink-0" />
				<span>{m.mods_panel_scan_failed({ message: scanError.message })}</span>
			</p>
		{/if}

		<div class="border-surface-700 flex gap-1 border-b" role="tablist">
			{#each tabs as entry (entry.id)}
				<button
					id={`${tabsId}-${entry.id}`}
					role="tab"
					aria-selected={tab === entry.id}
					aria-controls={`${tabsId}-panel`}
					tabindex={tab === entry.id ? 0 : -1}
					class={cn(
						'flex items-center gap-2 px-4 py-2.5 text-sm font-medium transition-colors',
						tab === entry.id
							? 'border-b-2'
							: 'text-surface-400 hover:text-surface-200 border-b-2 border-transparent'
					)}
					onclick={() => showTab(entry.id)}
					onkeydown={moveTab}
				>
					<Icon icon={entry.icon} size={16} />
					{entry.id === 'scan' && subscribedCount > 0
						? m.mods_tab_scan_count({ count: subscribedCount })
						: entry.label()}
				</button>
			{/each}
		</div>

		<div class="relative min-h-0 flex-1">
			<div
				id={`${tabsId}-panel`}
				class="h-full overflow-y-auto pb-28"
				role="tabpanel"
				tabindex="0"
				aria-labelledby={`${tabsId}-${tab}`}
			>
				{#if tab === 'mods'}
					<ModList {targetId} />
				{:else if tab === 'order'}
					<LoadOrderPanel {targetId} />
				{:else if tab === 'frameworks'}
					<FrameworksPanel {targetId} />
				{:else if tab === 'conflicts'}
					<ConflictsPanel {targetId} />
				{:else if tab === 'live'}
					<LivePanel {targetId} />
				{:else if tab === 'worlds'}
					<WorldsPanel {targetId} canLaunch={launchable} />
				{:else if tab === 'discover'}
					<DiscoverPanel {targetId} />
				{:else if tab === 'scan'}
					<ScanPanel {targetId} />
				{:else if tab === 'backups'}
					<BackupsPanel {targetId} />
				{/if}
			</div>
			<ApplyBar {targetId} onShowTab={showTab} />
		</div>
	</div>
{:else if noTarget}
	<Card class="text-surface-400 text-center text-sm">
		<p role="status">{m.mods_no_server_target()}</p>
	</Card>
{:else}
	<Card class="text-surface-400 flex items-center justify-center gap-2 text-sm">
		<Icon icon="tabler:loader-2" size={16} class="animate-spin" />
		<span role="status">{m.mods_preparing_server()}</span>
	</Card>
{/if}
