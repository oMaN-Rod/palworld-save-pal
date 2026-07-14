<script lang="ts">
	import * as m from '$i18n/messages';
	import { Button } from '$components/ui';
	import { slide } from 'svelte/transition';

	let { selectedCount, matchingCount, onSelectAll, onClear } = $props<{
		selectedCount: number;
		matchingCount: number;
		onSelectAll: () => void;
		onClear: () => void;
	}>();
</script>

{#if selectedCount > 0}
	<div class="bg-surface-900 flex items-center justify-between rounded-sm p-2 text-sm" transition:slide={{ duration: 200 }}>
		<span>{selectedCount} / {matchingCount}</span>
		<div class="flex gap-2">
			{#if selectedCount < matchingCount}
				<Button variant="ghost" onclick={onSelectAll}>
					{m.select_all_matching({ count: matchingCount })}
				</Button>
			{/if}
			<Button variant="ghost" onclick={onClear}>{m.clear_selection()}</Button>
		</div>
	</div>
{/if}
