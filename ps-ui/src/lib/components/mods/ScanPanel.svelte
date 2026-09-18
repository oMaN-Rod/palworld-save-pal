<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { untrack } from 'svelte';
	import { Button, Card } from '$components/ui';
	import { getModsState } from '$states';
	import { MessageType } from '$types';
	import type { AdoptionCandidate, FileState, ModError, RefusalSubject } from '$types';
	import * as m from '$i18n/messages';

	let { targetId }: { targetId: string } = $props();

	const modsState = getModsState();

	const lastScanned = $derived.by(() => {
		const at = modsState.lastFullScanAt[targetId];
		return at === undefined ? null : new Date(at).toLocaleString();
	});
	const report = $derived(modsState.scans[targetId]);
	const candidates = $derived(modsState.candidates[targetId] ?? []);
	const local = $derived(candidates.filter((candidate) => candidate.source === 'local'));
	const subscribed = $derived(
		candidates.filter((candidate) => candidate.source === 'steam_subscribed')
	);
	const warnings = $derived(modsState.scanWarnings[targetId] ?? []);
	const scanning = $derived(modsState.scanning[targetId] ?? false);
	const adopting = $derived(modsState.adopting[targetId] ?? false);

	const scanError = $derived(modsState.lastErrorFor(MessageType.MOD_TARGET_SCAN, targetId));

	function candidateSubject(candidate: AdoptionCandidate): RefusalSubject {
		return { candidate_name: candidate.name, source: candidate.source, root: candidate.root };
	}

	function adoptErrorFor(candidate: AdoptionCandidate): ModError | undefined {
		if (adopting) return undefined;
		return modsState.lastErrorFor(MessageType.MOD_ADOPT, targetId, candidateSubject(candidate));
	}

	/** Adopt refusals whose candidate is no longer listed have no row to sit under. */
	const unlistedAdoptErrors = $derived(
		adopting
			? []
			: modsState
					.lastErrorsFor(MessageType.MOD_ADOPT, targetId)
					.filter((error) => !candidates.some((candidate) => adoptErrorFor(candidate) === error))
	);

	const counted: { state: FileState; label: () => string }[] = [
		{ state: 'managed_intact', label: m.mods_scan_intact },
		{ state: 'managed_drifted', label: m.mods_scan_edited },
		{ state: 'managed_missing', label: m.mods_scan_missing },
		{ state: 'managed_unreadable', label: m.mods_scan_unreadable },
		{ state: 'unmanaged', label: m.mods_scan_unmanaged }
	];

	const counts = $derived.by(() => {
		const totals: Partial<Record<FileState, number>> = {};
		for (const file of report?.files ?? []) totals[file.state] = (totals[file.state] ?? 0) + 1;
		return totals;
	});

	const problems = $derived(
		report
			? [
					{ key: 'drifted', hint: m.mods_scan_edited_hint(), paths: report.drifted },
					{ key: 'missing', hint: m.mods_scan_missing_hint(), paths: report.missing },
					{ key: 'unreadable', hint: m.mods_scan_unreadable_hint(), paths: report.unreadable }
				].filter((problem) => problem.paths.length > 0)
			: []
	);

	const allManaged = $derived(
		modsState.scanReplyFull[targetId] === true &&
			!scanError &&
			candidates.length === 0 &&
			problems.length === 0 &&
			warnings.length === 0 &&
			(counts.unmanaged ?? 0) === 0
	);

	const kindLabels: Record<string, () => string> = {
		ue4ss: m.mods_panel_type_ue4ss,
		palschema: m.mods_panel_type_palschema,
		pak: m.mods_panel_type_pak,
		logicmods: m.mods_panel_type_logicmods,
		nativedll: m.mods_panel_type_nativedll,
		workshop: m.mods_panel_type_workshop,
		framework: m.mods_panel_type_framework
	};

	function candidateKey(candidate: AdoptionCandidate): string {
		return `${candidate.source}:${candidate.root}:${candidate.name}`;
	}

	function adoptErrorText(error: ModError): string {
		if (error.code === 'already_managed') return m.mods_scan_already_managed();
		if (error.code === 'not_a_candidate') return m.mods_scan_not_a_candidate();
		if (error.code === 'no_active_profile') return m.mods_apply_no_active_profile();
		return error.message;
	}

	$effect(() => {
		const id = targetId;
		untrack(() => modsState.scanOnce(id));
	});
</script>

{#snippet candidateRow(candidate: AdoptionCandidate)}
	{@const adoptError = adoptErrorFor(candidate)}
	<li class="flex flex-col gap-1 py-2">
		<div class="flex items-center justify-between gap-3">
			<div class="flex min-w-0 flex-col">
				<span class="truncate font-medium" title={candidate.root}>{candidate.name}</span>
				<span class="text-surface-400 flex flex-wrap gap-x-3 text-xs">
					<span>{kindLabels[candidate.kind]?.() ?? candidate.kind}</span>
					{#if candidate.source === 'steam_subscribed'}
						<span>{m.mods_scan_subscribed()}</span>
					{:else}
						<span>{m.mods_file_count({ count: candidate.files.length })}</span>
					{/if}
					<span>{candidate.enabled ? m.mods_panel_enabled() : m.mods_panel_disabled()}</span>
				</span>
			</div>
			<Button
				size="sm"
				class="shrink-0"
				disabled={adopting}
				onclick={() => modsState.adopt(targetId, candidate)}
			>
				{candidate.source === 'steam_subscribed' ? m.mods_panel_manage() : m.mods_scan_adopt()}
			</Button>
		</div>
		{#if adoptError}
			<div class="text-error-400 flex flex-col gap-0.5 text-sm" role="alert">
				<p>{adoptErrorText(adoptError)}</p>
				{#if adoptError.code === 'already_managed' && candidate.source === 'steam_subscribed'}
					<p class="text-surface-400 text-xs">{m.mods_scan_renamed_hint()}</p>
				{/if}
			</div>
		{/if}
	</li>
{/snippet}

<div class="flex flex-col gap-4">
	<div class="flex flex-wrap items-center justify-between gap-3">
		<div class="flex min-w-0 flex-col text-sm">
			{#if lastScanned}
				<span class="text-surface-300">{m.mods_scan_last({ date: lastScanned })}</span>
			{/if}
			<span class="text-surface-400 text-xs">{m.mods_scan_refresh_hint()}</span>
		</div>
		<Button
			size="sm"
			class="flex shrink-0 items-center gap-2"
			loading={scanning}
			onclick={() => modsState.scan(targetId, { full: true })}
		>
			{#if !scanning}<Icon icon="tabler:search" size={14} />{/if}
			{m.mods_scan_now()}
		</Button>
	</div>

	{#if scanError}
		<p class="text-error-400 flex items-start gap-2 text-sm" role="alert">
			<Icon icon="tabler:alert-circle" size={14} class="mt-0.5 shrink-0" />
			<span>{scanError.message}</span>
		</p>
	{/if}

	{#each unlistedAdoptErrors as error, index (index)}
		<p class="text-error-400 flex items-start gap-2 text-sm" role="alert">
			<Icon icon="tabler:alert-circle" size={14} class="mt-0.5 shrink-0" />
			<span>{adoptErrorText(error)}</span>
		</p>
	{/each}

	{#if warnings.includes('palmodsettings_unreadable')}
		<Card padding="p-3" class="border-warning-500/40 bg-warning-500/10 border text-sm">
			<p class="text-warning-400 flex items-start gap-2">
				<Icon icon="tabler:alert-triangle" size={14} class="mt-0.5 shrink-0" />
				<span>{m.mods_panel_palmodsettings_unreadable()}</span>
			</p>
		</Card>
	{/if}

	{#if report}
		<ul class="grid grid-cols-2 gap-2 sm:grid-cols-5">
			{#each counted as entry (entry.state)}
				<li data-state={entry.state}>
					<Card padding="p-3" class="flex flex-col">
						<span class="text-lg font-semibold" data-count>{counts[entry.state] ?? 0}</span>
						<span class="text-surface-400 text-xs">{entry.label()}</span>
					</Card>
				</li>
			{/each}
		</ul>

		{#each problems as problem (problem.key)}
			<details class="text-sm">
				<summary class="cursor-pointer select-none">
					<span>{problem.hint}</span>
					<span class="text-surface-400"> ({problem.paths.length})</span>
				</summary>
				<ul class="mt-1 flex flex-col gap-0.5 font-mono text-xs">
					{#each problem.paths as path (path)}
						<li class="truncate" title={path}>{path}</li>
					{/each}
				</ul>
			</details>
		{/each}
	{:else}
		<p class="text-surface-400 text-sm">{m.mods_scan_prompt()}</p>
	{/if}

	{#if local.length > 0}
		<section class="flex flex-col gap-1">
			<h3 class="text-sm font-semibold">{m.mods_scan_candidates_title()}</h3>
			<p class="text-surface-400 text-xs">{m.mods_scan_adopt_hint()}</p>
			<ul class="divide-surface-700 divide-y">
				{#each local as candidate (candidateKey(candidate))}
					{@render candidateRow(candidate)}
				{/each}
			</ul>
		</section>
	{/if}

	{#if subscribed.length > 0}
		<section class="flex flex-col gap-1">
			<h3 class="text-sm font-semibold">{m.mods_panel_subscribed_title()}</h3>
			<p class="text-surface-400 text-xs">{m.mods_panel_subscribed_hint()}</p>
			<ul class="divide-surface-700 divide-y">
				{#each subscribed as candidate (candidateKey(candidate))}
					{@render candidateRow(candidate)}
				{/each}
			</ul>
		</section>
	{/if}

	{#if allManaged}
		<Card padding="p-4" class="text-success-400 flex items-center gap-2 text-sm">
			<Icon icon="tabler:circle-check" size={16} />
			{m.mods_scan_all_managed()}
		</Card>
	{/if}
</div>
