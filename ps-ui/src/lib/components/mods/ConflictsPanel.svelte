<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { untrack } from 'svelte';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { Button, Card, Spinner } from '$components/ui';
	import { getModsState } from '$states';
	import { MessageType } from '$types';
	import type { LibraryMod, ModConflict, ModVersion } from '$types';
	import * as m from '$i18n/messages';
	import {
		conflictErrorText,
		conflictSummary,
		refLabel,
		unreadableReasonText
	} from './conflictText';
	import { displayName, currentVersion } from './modList';
	import { canConvertIostore } from './modCapabilities';
	import { iostoreErrorText } from './iostoreText';

	let { targetId }: { targetId: string } = $props();

	const modsState = getModsState();
	const remoteMode = getRemoteMode();
	const mode = $derived({ desktop: PUBLIC_DESKTOP_MODE === 'true', remote: remoteMode.active });

	const target = $derived(modsState.targets.find((entry) => entry.id === targetId));
	const report = $derived(modsState.conflicts[targetId]);
	const checking = $derived(modsState.checkingConflicts[targetId] ?? false);
	const refusal = $derived(modsState.lastErrorFor(MessageType.MOD_CONFLICTS, targetId));
	const hasConflicts = $derived((report?.conflicts.length ?? 0) > 0);

	function modOf(modId: string) {
		return modsState.mods.find((entry) => entry.id === modId);
	}

	const reportProfile = $derived(
		report
			? (modsState.profiles[targetId] ?? []).find((entry) => entry.id === report.profile_id)
			: undefined
	);

	/** The version the report scanned: the profile's pin, or else the mod's current version. */
	function scannedVersion(mod: LibraryMod): ModVersion | undefined {
		const pinned = reportProfile?.mods.find((entry) => entry.mod_id === mod.id)?.mod_version_id;
		return pinned ? mod.versions.find((entry) => entry.id === pinned) : currentVersion(mod);
	}

	function name(modId: string): string {
		const mod = modsState.mods.find((entry) => entry.id === modId);
		return mod ? displayName(mod) : modId;
	}

	function byKind<K extends ModConflict['kind']>(
		kind: K,
		conflicts: ModConflict[]
	): Extract<ModConflict, { kind: K }>[] {
		return conflicts.filter(
			(entry): entry is Extract<ModConflict, { kind: K }> => entry.kind === kind
		);
	}

	const sections = $derived.by(() => {
		const conflicts = report?.conflicts ?? [];
		return [
			{ label: m.mods_conflicts_section_missing, items: byKind('missing_dependency', conflicts) },
			{
				label: m.mods_conflicts_section_gamepass,
				items: byKind('gamepass_pak_incompatible', conflicts)
			},
			{ label: m.mods_conflicts_section_overlap, items: byKind('pak_overlap', conflicts) },
			{ label: m.mods_conflicts_section_palschema, items: byKind('palschema_row', conflicts) }
		].filter((section) => section.items.length > 0);
	});

	/** A remount with an already-stored report skips the request; a target change or a reset never does. */
	let requested: { id: string; resets: number } | undefined;

	$effect(() => {
		const id = targetId;
		const resets = modsState.resets;
		untrack(() => {
			const first = requested === undefined;
			requested = { id, resets };
			if (first && modsState.conflicts[id]) return;
			modsState.loadConflicts(id);
		});
	});
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center justify-end">
		<Button
			variant="ghost"
			size="sm"
			class="flex items-center gap-2"
			disabled={checking}
			onclick={() => modsState.loadConflicts(targetId)}
		>
			<Icon icon="tabler:refresh" size={14} />
			{m.mods_conflicts_check_again()}
		</Button>
	</div>

	{#if refusal}
		<p class="text-error-400 text-sm" role="alert">{conflictErrorText(refusal)}</p>
	{/if}

	{#if checking && !report}
		<Card padding="p-6" class="text-surface-400 flex items-center gap-3 text-sm">
			<Spinner size="size-4" />
			{m.mods_conflicts_checking()}
		</Card>
	{:else if report}
		{#if !hasConflicts}
			<Card padding="p-6" class="text-surface-400 flex items-center gap-3 text-sm">
				<Icon icon="tabler:circle-check" size={20} class="shrink-0" />
				{m.mods_conflicts_none()}
			</Card>
		{:else}
			{#each sections as section (section.label)}
				<div class="flex flex-col gap-2">
					<h3 class="text-surface-300 text-xs font-medium uppercase">
						{section.label()} ({section.items.length})
					</h3>
					{#each section.items as conflict, index (index)}
						<Card padding="p-3" class="text-sm">
							<p>{conflictSummary(conflict, name)}</p>
							{#if conflict.kind === 'pak_overlap'}
								{#if conflict.paks.some((pak) => pak.mod_id === null)}
									<p class="text-surface-400 mt-1 text-xs">{m.mods_conflict_untracked_hint()}</p>
								{/if}
								<details class="mt-2 text-xs">
									<summary class="text-surface-400 cursor-pointer select-none">
										{m.mods_conflict_assets({
											count: conflict.asset_count,
											winner: refLabel(conflict.winner, name)
										})}
									</summary>
									<ul class="mt-1 flex flex-col gap-0.5">
										{#each conflict.assets.slice(0, 10) as asset (asset)}
											<li class="text-surface-400 truncate font-mono" title={asset}>{asset}</li>
										{/each}
									</ul>
								</details>
							{/if}
							{#if conflict.kind === 'gamepass_pak_incompatible'}
								{@const convertMod = modOf(conflict.mod_id)}
								{@const convertError = modsState.lastErrorFor(
									MessageType.MOD_IOSTORE_CONVERT,
									targetId,
									{ mod_id: conflict.mod_id }
								)}
								{#if target && convertMod && canConvertIostore(target, mode, convertMod, scannedVersion(convertMod))}
									<Button
										size="sm"
										variant="ghost"
										class="mt-2"
										disabled={modsState.converting[targetId] ?? false}
										onclick={() => modsState.convertIostore(targetId, conflict.mod_id)}
									>
										{m.mods_conflict_convert({ mod: name(conflict.mod_id) })}
									</Button>
								{/if}
								{#if convertError}
									<p class="text-error-400 mt-2 flex items-center gap-2 text-xs" role="alert">
										<Icon icon="tabler:alert-circle" size={12} />
										{iostoreErrorText(convertError)}
									</p>
								{/if}
							{/if}
						</Card>
					{/each}
				</div>
			{/each}
		{/if}

		{#if report.unreadable.length > 0}
			<details class="text-sm">
				<summary class="text-surface-300 cursor-pointer select-none">
					{m.mods_conflicts_unreadable({ count: report.unreadable.length })}
				</summary>
				<ul class="mt-2 flex flex-col gap-1">
					{#each report.unreadable as entry, index (index)}
						<li class="text-surface-400 text-xs" title={entry.path ?? undefined}>
							{#if entry.mod_id === null}
								{refLabel(entry, name)}: {unreadableReasonText(entry.reason)}
							{:else}
								{name(entry.mod_id)} — {entry.file}: {unreadableReasonText(entry.reason)}
							{/if}
						</li>
					{/each}
				</ul>
			</details>
		{/if}
	{/if}
</div>
