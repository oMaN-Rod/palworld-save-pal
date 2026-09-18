<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Component } from 'svelte';
	import { Button } from '$components/ui';
	import { getModalState, getModsState } from '$states';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import ApplyResultModal from './ApplyResultModal.svelte';
	import type { ModsTab } from './TargetPanel.svelte';
	import { isNotable, pendingTotal } from './applyOutcome';

	let { targetId, onShowTab }: { targetId: string; onShowTab: (tab: ModsTab) => void } = $props();

	const modsState = getModsState();
	const modal = getModalState();

	const plan = $derived(modsState.plans[targetId]);
	const total = $derived(plan ? pendingTotal(plan.counts) : 0);
	const refused = $derived(
		modsState.lastErrorFor(MessageType.PROFILE_PLAN, targetId) !== undefined
	);
	const applying = $derived(
		(modsState.applying[targetId] ?? false) || modsState.progressFor(targetId) !== undefined
	);
	const last = $derived(modsState.lastApply[targetId]);

	const pill = 'flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium';

	function reopen() {
		if (!last) return;
		modal.showModal(ApplyResultModal as unknown as Component, {
			result: last,
			targetId,
			onShowTab
		});
	}
</script>

{#if last && isNotable(last) && !applying}
	<Button
		variant="ghost"
		size="sm"
		aria-label={m.mods_apply_last_result()}
		title={m.mods_apply_last_result()}
		onclick={reopen}
	>
		<Icon icon="tabler:history" size={16} />
	</Button>
{/if}
{#if applying}
	<span class={[pill, 'bg-surface-700 text-surface-200']} role="status">
		<Icon icon="tabler:loader-2" size={12} class="animate-spin" />
		{m.mods_apply_stage_applying()}
	</span>
{:else if refused}
	<span class={[pill, 'bg-error-500/20 text-error-400']}>
		<Icon icon="tabler:alert-circle" size={12} />
		{m.mods_apply_status_blocked()}
	</span>
{:else if total > 0}
	<span class={[pill, 'bg-warning-500/15 text-warning-400']}>
		<Icon icon="tabler:clock" size={12} />
		{m.mods_apply_status_pending_count({ count: total })}
	</span>
{:else if modsState.pending[targetId]}
	<span class={[pill, 'bg-warning-500/15 text-warning-400']}>
		<Icon icon="tabler:clock" size={12} />
		{m.mods_apply_status_pending()}
	</span>
{:else if plan}
	<span class={[pill, 'bg-success-500/15 text-success-400']}>
		<Icon icon="tabler:circle-check" size={12} />
		{m.mods_apply_up_to_date()}
	</span>
{/if}
