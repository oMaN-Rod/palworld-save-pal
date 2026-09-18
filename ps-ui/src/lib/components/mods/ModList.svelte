<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { untrack, type Component } from 'svelte';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { Button, Card, Input, Select } from '$components/ui';
	import { getModalState, getModsState, getNexusState } from '$states';
	import InstallModal from './InstallModal.svelte';
	import InactiveProfileNotice from './InactiveProfileNotice.svelte';
	import ModCard from './ModCard.svelte';
	import ModDetailDrawer from './ModDetailDrawer.svelte';
	import { modsViewMode, type ModsView } from './modsView.svelte';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import { nexusErrorText } from './nexusText';
	import { modTypeLabel as typeLabel } from './modLabels';
	import { filterMods, inUseProfileIds, sortMods, type ModSort } from './modList';

	let { targetId }: { targetId: string } = $props();

	type StatusFilter = 'all' | 'enabled' | 'disabled';

	const modsState = getModsState();
	const nexusState = getNexusState();
	const modal = getModalState();
	const remoteMode = getRemoteMode();
	const mode = $derived({ desktop: PUBLIC_DESKTOP_MODE === 'true', remote: remoteMode.active });

	function openInstall() {
		modal.showModal<boolean>(InstallModal as unknown as Component, { targetId });
	}

	const profile = $derived(modsState.viewedProfile(targetId));

	let query = $state('');
	let sort = $state<ModSort>('name');
	let status = $state<StatusFilter>('all');
	let type = $state<string | null>(null);
	let selectedId = $state<string | null>(null);
	const requestedProfiles = new Set<string>();

	const sortOptions = $derived([
		{ label: m.mods_list_sort_name(), value: 'name' },
		{ label: m.mods_list_sort_type(), value: 'type' },
		{ label: m.mods_list_sort_size(), value: 'size' }
	]);

	const statusOptions: { value: StatusFilter; label: () => string }[] = [
		{ value: 'all', label: m.mods_filter_all },
		{ value: 'enabled', label: m.mods_panel_enabled },
		{ value: 'disabled', label: m.mods_panel_disabled }
	];

	const views: { value: ModsView; label: () => string; icon: string }[] = [
		{ value: 'grid', label: m.mods_view_grid, icon: 'tabler:layout-grid' },
		{ value: 'list', label: m.mods_view_list, icon: 'tabler:list' }
	];

	const types = $derived(
		[...new Set(modsState.mods.map((mod) => mod.mod_type))].sort((a, b) =>
			typeLabel(a).localeCompare(typeLabel(b))
		)
	);

	const visible = $derived.by(() => {
		const enabledIn = (modId: string) =>
			profile ? modsState.enabledIn(targetId, profile.id, modId) : false;
		const matching = filterMods(modsState.mods, query).filter(
			(mod) =>
				(type === null || mod.mod_type === type) &&
				(status === 'all' || enabledIn(mod.id) === (status === 'enabled'))
		);
		return sortMods(matching, sort, typeLabel);
	});

	const selected = $derived(modsState.mods.find((mod) => mod.id === selectedId));

	const updateRun = $derived(nexusState.updateRuns[targetId]);
	const updateCheckError = $derived(modsState.lastErrorFor(MessageType.MOD_UPDATE_CHECK, targetId));

	$effect(() => {
		targetId;
		modsState.resets;
		untrack(() => {
			if (mode.desktop && !mode.remote) nexusState.checkUpdates(targetId);
		});
	});

	/** A toggle refused before the server looked the mod up names no row. */
	const targetRefusals = $derived(
		modsState
			.lastErrorsFor(MessageType.PROFILE_SET_MOD, targetId)
			.filter((error) => error.subject === undefined)
			.map((error) =>
				error.code === 'target_not_found' ? m.mods_list_target_missing() : error.message
			)
	);

	$effect(() => {
		const named = [MessageType.MOD_REMOVE, MessageType.MOD_VERSION_DELETE].flatMap((type) => {
			const error = modsState.lastErrorFor(type);
			return error?.code === 'version_in_use' ? inUseProfileIds(error) : [];
		});
		const known = new Set(
			Object.values(modsState.profiles)
				.flat()
				.map((entry) => entry.id)
		);
		if (named.every((id) => known.has(id))) return;
		const unloaded = modsState.targets.filter(
			(entry) => !(entry.id in modsState.profiles) && !requestedProfiles.has(entry.id)
		);
		untrack(() => {
			for (const entry of unloaded) {
				requestedProfiles.add(entry.id);
				modsState.loadProfiles(entry.id);
			}
		});
	});

	const chip =
		'rounded-full border px-3 py-1 text-xs font-medium transition-colors aria-pressed:border-surface-500 aria-pressed:bg-surface-700 aria-pressed:text-surface-50 border-surface-800 bg-surface-900 text-surface-400 hover:text-surface-200';
</script>

<div class="flex flex-col gap-3">
	{#if profile}
		<InactiveProfileNotice {profile} />
	{/if}
	{#each targetRefusals as line}
		<p class="text-error-400 flex items-center gap-2 text-sm" role="alert">
			<Icon icon="tabler:alert-circle" size={14} class="shrink-0" />
			{line}
		</p>
	{/each}
	{#if mode.desktop && !mode.remote && updateCheckError}
		<p class="text-error-400 flex items-center gap-2 text-sm" role="alert">
			<Icon icon="tabler:alert-circle" size={14} class="shrink-0" />
			{nexusErrorText(updateCheckError)}
		</p>
	{/if}
	{#if modsState.mods.length === 0}
		<Card class="text-surface-400 text-center">
			<Icon icon="tabler:package" size={32} class="mx-auto mb-2 opacity-50" />
			<p>{m.mods_panel_empty()}</p>
			<p class="mt-1 text-xs">{m.mods_panel_empty_hint()}</p>
			<Button variant="primary" size="sm" class="mx-auto mt-3" onclick={openInstall}>
				<Icon icon="tabler:upload" size={14} />
				{m.mods_install_open()}
			</Button>
		</Card>
	{:else}
		<div class="flex flex-wrap items-end gap-3">
			<Button variant="primary" size="sm" class="mb-2" onclick={openInstall}>
				<Icon icon="tabler:upload" size={14} />
				{m.mods_install_open()}
			</Button>
			<div class="min-w-48 flex-1">
				<Input
					type="search"
					placeholder={m.mods_list_filter_placeholder()}
					bind:value={query}
					inputClass="w-full"
				/>
			</div>
			{#if mode.desktop && !mode.remote}
				<Button
					variant="ghost"
					size="sm"
					class="mb-2"
					disabled={nexusState.isBusy('checkingUpdates')}
					onclick={() => nexusState.checkUpdates(targetId, { force: true })}
				>
					<Icon icon="tabler:refresh" size={14} />
					{m.mods_update_check_again()}
				</Button>
				{#if nexusState.isBusy('checkingUpdates')}
					<span class="text-surface-400 mb-2 text-xs">{m.mods_update_checking()}</span>
				{:else if updateRun}
					<span class="text-surface-400 mb-2 text-xs">
						{m.mods_update_checked({ count: updateRun.checked })}
					</span>
				{/if}
			{/if}
			<div class="w-40">
				<Select
					label={m.mods_list_sort_label()}
					options={sortOptions}
					value={sort}
					onChange={(value: string | number) => (sort = value as ModSort)}
				/>
			</div>
			<div
				class="border-surface-800 bg-surface-900 mb-2 flex rounded-sm border p-0.5"
				role="group"
				aria-label={m.mods_view_label()}
			>
				{#each views as view (view.value)}
					<button
						class="text-surface-400 hover:text-surface-200 aria-pressed:bg-surface-700 aria-pressed:text-surface-50 rounded-xs p-1.5"
						aria-pressed={modsViewMode.current === view.value}
						aria-label={view.label()}
						title={view.label()}
						onclick={() => (modsViewMode.current = view.value)}
					>
						<Icon icon={view.icon} size={16} />
					</button>
				{/each}
			</div>
		</div>

		{#if mode.desktop && !mode.remote && updateRun?.truncated}
			<p class="text-surface-400 text-xs">{m.mods_update_truncated()}</p>
		{/if}

		<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
			<div class="flex gap-1" role="group" aria-label={m.mods_filter_status()}>
				{#each statusOptions as option (option.value)}
					<button
						class={chip}
						aria-pressed={status === option.value}
						onclick={() => (status = option.value)}
					>
						{option.label()}
					</button>
				{/each}
			</div>
			{#if types.length > 1}
				<div class="flex flex-wrap gap-1" role="group" aria-label={m.mods_filter_type()}>
					<button class={chip} aria-pressed={type === null} onclick={() => (type = null)}>
						{m.mods_filter_all_types()}
					</button>
					{#each types as entry (entry)}
						<button class={chip} aria-pressed={type === entry} onclick={() => (type = entry)}>
							{typeLabel(entry)}
						</button>
					{/each}
				</div>
			{/if}
		</div>

		{#if visible.length === 0}
			<p class="text-surface-400 py-4 text-center text-sm">{m.mods_list_no_match()}</p>
		{/if}

		<div class={['grid items-start gap-4', selected && 'lg:grid-cols-[minmax(0,1fr)_20rem]']}>
			<div
				data-view={modsViewMode.current}
				class={modsViewMode.current === 'grid'
					? 'grid grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-3'
					: 'flex flex-col gap-2'}
			>
				{#each visible as mod (mod.id)}
					<ModCard
						{mod}
						{targetId}
						layout={modsViewMode.current}
						selected={mod.id === selectedId}
						onDetails={(modId) => (selectedId = modId)}
					/>
				{/each}
			</div>
			{#if selected}
				<div class="order-first lg:order-none">
					<ModDetailDrawer mod={selected} {targetId} onClose={() => (selectedId = null)} />
				</div>
			{/if}
		</div>
	{/if}
</div>
