<script lang="ts">
	import UPSPalBadge from './UPSPalBadge.svelte';
	import { getUpsState } from '$states';
	import type { UPSPal } from '$types';

	const upsState = getUpsState();

	function handlePalSelect(upsPal: UPSPal, event: MouseEvent) {
		// Only handle selection when Ctrl+click (following other storage systems pattern)
		if (event.ctrlKey || event.metaKey) {
			upsState.togglePalSelection(upsPal.id);
		}
	}

	function isPalSelected(palId: number): boolean {
		return upsState.selectedPals.has(palId);
	}
</script>

<div class="p-4">
	<div
		class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-8"
	>
		{#each upsState.pals as upsPal (upsPal.id)}
			<div class="relative">
				<!-- UPS Pal Badge with context menu -->
				<div class="relative">
					<UPSPalBadge {upsPal} onSelect={handlePalSelect} />
				</div>

				<!-- UPS-specific overlay info -->
				<div class="absolute bottom-2 right-2">
					{#if upsPal.tags && upsPal.tags.length > 0}
						<div class="rounded bg-black/70 px-2 py-1 text-xs text-white">
							{upsPal.tags.length}🏷️
						</div>
					{/if}
				</div>

				<!-- Additional stats overlay -->
				<div class="absolute right-2 top-2 text-right">
					{#if upsPal.transfer_count > 0 || upsPal.clone_count > 0}
						<div class="space-y-1 rounded bg-black/70 px-2 py-1 text-xs text-white">
							{#if upsPal.transfer_count > 0}
								<div title="Transfer count">📤{upsPal.transfer_count}</div>
							{/if}
							{#if upsPal.clone_count > 0}
								<div title="Clone count">🔄{upsPal.clone_count}</div>
							{/if}
						</div>
					{/if}
				</div>
			</div>
		{/each}
	</div>
</div>
