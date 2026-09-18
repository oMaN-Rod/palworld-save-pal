<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { untrack, type Component } from 'svelte';
	import { Button, Progress } from '$components/ui';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { getModalState, getModsState, getToastState } from '$states';
	import { MessageType } from '$types';
	import type { ModProgress } from '$types';
	import * as m from '$i18n/messages';
	import ApplyResultModal from './ApplyResultModal.svelte';
	import type { ModsTab } from './TargetPanel.svelte';
	import {
		confirmReplace,
		errorText,
		isNotable,
		joinList,
		pendingParts,
		pendingTotal,
		stringList
	} from './applyOutcome';

	let { targetId, onShowTab }: { targetId: string; onShowTab: (tab: ModsTab) => void } = $props();

	const modsState = getModsState();
	const modal = getModalState();
	const toasts = getToastState();
	const remoteMode = getRemoteMode();
	const stageId = $props.id();

	const plan = $derived(modsState.plans[targetId]);
	const total = $derived(plan ? pendingTotal(plan.counts) : 0);
	const summary = $derived(plan ? joinList(pendingParts(plan.counts), 'unit') : '');
	const edited = $derived(plan?.counts.preserve ?? 0);
	const pending = $derived(modsState.pending[targetId] ?? false);
	const progress = $derived(modsState.progressFor(targetId));
	const applying = $derived(modsState.applying[targetId] ?? false);
	const busy = $derived(
		applying || (modsState.settingMod[targetId] ?? false) || progress !== undefined
	);
	const planError = $derived(modsState.lastErrorFor(MessageType.PROFILE_PLAN, targetId));
	const planOccupants = $derived(
		planError?.code === 'unmanaged_occupant' ? stringList(planError.paths) : []
	);
	const canApply = $derived(!busy && (pending || total > 0 || planError !== undefined));
	/** Progress alone doesn't show the bar: a toggle on the active profile applies in the background. */
	const visible = $derived(pending || total > 0 || planError !== undefined || applying);

	const stageText: Partial<Record<ModProgress['stage'], () => string>> = {
		checking: m.mods_apply_stage_checking,
		applying: m.mods_apply_stage_applying,
		resolving: m.mods_progress_resolving,
		downloading: m.mods_progress_downloading,
		storing: m.mods_progress_storing,
		converting: m.mods_progress_converting
	};

	/** The request each target last announced; an outcome already present when a target is first shown is old news. */
	const announced = new Map<string, string | undefined>();
	/** Targets with an apply started from the bar, whose plain success is worth a toast. */
	const requested = new Set<string>();

	$effect(() => {
		if (applying) requested.add(targetId);
	});

	$effect(() => {
		const last = modsState.lastApply[targetId];
		if (!announced.has(targetId)) {
			announced.set(targetId, last?.request_id);
			return;
		}
		if (!last || busy || announced.get(targetId) === last.request_id) return;
		announced.set(targetId, last.request_id);
		const wasRequested = requested.delete(targetId);
		untrack(() => {
			if (isNotable(last)) {
				modal.showModal(ApplyResultModal as unknown as Component, {
					result: last,
					targetId,
					onShowTab
				});
			} else if (wasRequested) {
				toasts.add(
					m.mods_apply_result_success({ count: pendingTotal(last.counts) }),
					undefined,
					'success'
				);
			}
		});
	});

	function adopt() {
		modsState.scan(targetId);
		onShowTab('scan');
	}

	async function replace(paths: string[]) {
		if (await confirmReplace(modal, remoteMode.active)) modsState.apply(targetId, paths);
	}
</script>

{#if visible}
	<div class="pointer-events-none absolute inset-x-0 bottom-4 z-10 flex justify-center px-4">
		<section
			aria-label={m.mods_apply_region()}
			class={[
				'bg-surface-800 pointer-events-auto relative flex w-full max-w-2xl flex-col gap-2 overflow-hidden rounded-xl border px-4 py-3 shadow-2xl shadow-black/50',
				planError ? 'border-error-500/50' : 'border-surface-600'
			]}
		>
			<div class="flex flex-wrap items-center gap-3">
				<Icon
					icon={planError ? 'tabler:alert-circle' : progress ? 'tabler:loader-2' : 'tabler:clock'}
					size={20}
					class={[
						'shrink-0',
						planError ? 'text-error-400' : progress ? 'animate-spin' : 'text-warning-400'
					]}
				/>
				<div class="flex min-w-0 flex-1 flex-col gap-0.5 text-sm">
					{#if pending}
						<span class="text-warning-400">{m.mods_list_pending()}</span>
					{/if}
					{#if planError}
						<p class="text-error-400" role="status">{errorText(planError, modsState.mods)}</p>
					{:else if total > 0}
						<span class="font-semibold">{m.mods_apply_pending({ count: total })}</span>
						<span class="text-surface-400 text-xs">{summary}</span>
					{/if}
					{#if edited > 0 && !planError}
						<span class="text-surface-400 text-xs"
							>{m.mods_apply_edited_kept({ count: edited })}</span
						>
					{/if}
				</div>
				{#if planOccupants.length > 0}
					<Button size="sm" variant="ghost" onclick={() => replace(planOccupants)}>
						{m.mods_apply_replace()}
					</Button>
					<Button size="sm" variant="outline" onclick={adopt}>{m.mods_apply_adopt()}</Button>
				{/if}
				<Button
					size="sm"
					variant="primary"
					disabled={!canApply}
					loading={busy}
					onclick={() => modsState.apply(targetId)}
				>
					{m.mods_panel_apply()}
				</Button>
			</div>

			{#if planOccupants.length > 0}
				<ul
					class="text-surface-300 flex max-h-24 flex-col gap-0.5 overflow-y-auto pl-8 font-mono text-xs"
				>
					{#each planOccupants as path (path)}
						<li class="truncate" title={path}>{path}</li>
					{/each}
				</ul>
			{/if}

			{#if progress}
				<div class="flex flex-col gap-1 pl-8" role="group" aria-labelledby={stageId}>
					<span id={stageId} class="text-surface-300 text-xs" aria-live="polite">
						{stageText[progress.stage]?.() ?? progress.message}
					</span>
					<Progress value={progress.pct} max={100} showLabel={false} rounded="rounded-full" />
				</div>
			{/if}
		</section>
	</div>
{/if}
