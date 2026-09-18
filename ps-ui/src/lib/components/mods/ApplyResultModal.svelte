<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button } from '$components/ui';
	import { cn } from '$theme';
	import type { ApplyResult as ApplyReply } from '$types';
	import * as m from '$i18n/messages';
	import ApplyResult from './ApplyResult.svelte';
	import type { ModsTab } from './TargetPanel.svelte';
	import { appliedStats, isFailed } from './applyOutcome';

	let {
		result,
		targetId,
		onShowTab,
		closeModal
	}: {
		result: ApplyReply;
		targetId: string;
		onShowTab: (tab: ModsTab) => void;
		closeModal: () => void;
	} = $props();

	const failed = $derived(isFailed(result));
	const stats = $derived(failed ? [] : appliedStats(result.counts));
	const title = $derived(
		result.error
			? m.mods_apply_result_failed_title()
			: result.mid_apply
				? m.mods_apply_result_unfinished_title()
				: m.mods_apply_result_notes_title()
	);

	function showTab(tab: ModsTab) {
		closeModal();
		onShowTab(tab);
	}
</script>

<div
	class="bg-surface-900 border-surface-700 flex max-h-[85vh] w-[520px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-lg border shadow-2xl"
>
	<div class="flex items-start gap-3 p-5 pb-4">
		<div
			class={cn(
				'flex size-9 shrink-0 items-center justify-center rounded-lg',
				failed ? 'bg-error-500/15 text-error-400' : 'bg-warning-500/15 text-warning-400'
			)}
		>
			<Icon icon={failed ? 'tabler:alert-circle' : 'tabler:alert-triangle'} size={20} />
		</div>
		<h3 class="pt-1.5 text-base font-bold">{title}</h3>
	</div>

	{#if stats.length > 0}
		<dl
			class="bg-surface-800 border-surface-800 grid gap-px border-y"
			style:grid-template-columns={`repeat(${stats.length}, minmax(0, 1fr))`}
		>
			{#each stats as stat (stat.label)}
				<div class="bg-surface-900 flex flex-col-reverse px-5 py-2.5">
					<dt class="text-surface-400 text-xs">{stat.label}</dt>
					<dd class="text-xl font-bold tabular-nums">{stat.count}</dd>
				</div>
			{/each}
		</dl>
	{/if}

	<div class="min-h-0 overflow-y-auto px-5 py-4">
		<ApplyResult {result} {targetId} framed={false} onShowTab={showTab} onAction={closeModal} />
	</div>

	<div class="border-surface-800 bg-surface-950 flex justify-end border-t px-5 py-3">
		<Button variant="primary" size="sm" onclick={() => closeModal()} data-modal-primary>
			{m.mods_apply_result_done()}
		</Button>
	</div>
</div>
