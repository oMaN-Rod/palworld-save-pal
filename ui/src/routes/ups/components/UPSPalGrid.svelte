<script lang="ts">
	import UPSPalBadge from './UPSPalBadge.svelte';
	import { getUpsState } from '$states';
	import type { UPSPal } from '$types';

	const upsState = getUpsState();

	function handlePalClick(upsPal: UPSPal) {
		upsState.togglePalSelection(upsPal.id);
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
			<div
				class="relative"
				role="button"
				tabindex="0"
				onclick={() => handlePalClick(upsPal)}
				onkeydown={(e) => {
					if (e.key === 'Enter' || e.key === ' ') {
						e.preventDefault();
						handlePalClick(upsPal);
					}
				}}
			>
				<!-- Selection Indicator -->
				{#if isPalSelected(upsPal.id)}
					<div class="bg-primary-500 absolute -inset-1 z-0 rounded-lg opacity-50"></div>
				{/if}

				<!-- UPS Pal Badge with context menu -->
				<div class="relative z-10">
					<UPSPalBadge {upsPal} onSelect={handlePalClick} />
				</div>

				<!-- Selection checkbox -->
				<div class="absolute left-2 top-2 z-20">
					<input
						type="checkbox"
						checked={isPalSelected(upsPal.id)}
						onchange={() => handlePalClick(upsPal)}
						class="checked:bg-primary-500 checked:border-primary-500 h-4 w-4 rounded border-2 border-white bg-black/50"
					/>
				</div>

				<!-- UPS-specific overlay info -->
				<div class="absolute bottom-2 right-2 z-10">
					{#if upsPal.tags && upsPal.tags.length > 0}
						<div class="rounded bg-black/70 px-2 py-1 text-xs text-white">
							{upsPal.tags.length}🏷️
						</div>
					{/if}
				</div>

				<!-- Additional stats overlay -->
				<div class="absolute right-2 top-2 z-10 text-right">
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
