<script lang="ts">
	import UPSPalBadge from './UPSPalBadge.svelte';
	import { getUpsState } from '$states';
	import type { UPSPal } from '$types';
	import Tag from '@lucide/svelte/icons/tag';
	import Upload from '@lucide/svelte/icons/upload';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import * as m from '$i18n/messages';

	const upsState = getUpsState();

	function handlePalSelect(upsPal: UPSPal, event: MouseEvent) {
		if (event.ctrlKey || event.metaKey) {
			upsState.togglePalSelection(upsPal.id);
		}
	}
</script>

<div class="p-6">
	<div
		class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-8"
	>
		{#each upsState.pals as upsPal (upsPal.id)}
			<div class="relative">
				<div class="relative">
					<UPSPalBadge {upsPal} onSelect={handlePalSelect} />
				</div>

				<div class="absolute right-2 bottom-2">
					{#if upsPal.tags && upsPal.tags.length > 0}
						<div class="rounded bg-black/70 px-2 py-1 text-xs text-white">
							{upsPal.tags.length}<Tag size={10} class="ml-0.5 inline" />
						</div>
					{/if}
				</div>

				<div class="absolute top-2 right-2 text-right">
					{#if upsPal.transfer_count > 0 || upsPal.clone_count > 0}
						<div class="space-y-1 rounded bg-black/70 px-2 py-1 text-xs text-white">
							{#if upsPal.transfer_count > 0}
								<div title={m.transfer({ count: 2 })}>
									<Upload size={10} class="mr-0.5 inline" />{upsPal.transfer_count}
								</div>
							{/if}
							{#if upsPal.clone_count > 0}
								<div title={m.clones()}>
									<RefreshCw size={10} class="mr-0.5 inline" />{upsPal.clone_count}
								</div>
							{/if}
						</div>
					{/if}
				</div>
			</div>
		{/each}
	</div>
</div>
