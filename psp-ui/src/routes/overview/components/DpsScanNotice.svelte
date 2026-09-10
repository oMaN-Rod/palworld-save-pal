<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button } from '$components/ui';
	import { getAppState, getOverviewState } from '$states';
	import * as m from '$i18n/messages';

	let { pending }: { pending: number } = $props();

	const appState = getAppState();
	const overviewState = getOverviewState();
</script>

<div
	class="border-surface-600/60 bg-surface-900/40 flex flex-wrap items-center gap-3 rounded-md border px-4 py-3"
>
	<Icon icon="tabler:box-multiple" size={18} class="text-secondary-400 shrink-0" />
	<p class="text-surface-300 min-w-0 flex-1 text-sm">
		{#if overviewState.scanningDps && appState.progressMessage}
			{appState.progressMessage}
		{:else}
			{m.overview_dps_unscanned({ count: pending.toLocaleString() })}
		{/if}
	</p>
	<Button
		variant="outline"
		size="sm"
		loading={overviewState.scanningDps}
		onclick={() => overviewState.scanDps()}
	>
		<Icon icon="tabler:scan" size={14} />
		{m.overview_dps_scan()}
	</Button>
</div>
