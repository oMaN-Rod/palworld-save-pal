<script lang="ts">
	import UPSPalBadge from './UPSPalBadge.svelte';
	import { getUpsState } from '$states';
	import type { UPSPal } from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$utils/commonTranslations';

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

	function formatDate(dateString: string): string {
		return new Date(dateString).toLocaleString();
	}
</script>

<div class="space-y-2 p-4">
	{#each upsState.pals as upsPal (upsPal.id)}
		<div
			class="border-surface-300 dark:border-surface-700 hover:border-primary-500 dark:hover:border-primary-400 flex cursor-pointer items-center gap-4 rounded-lg border p-3 transition-colors {isPalSelected(
				upsPal.id
			)
				? 'bg-primary-50 dark:bg-primary-900/20 border-primary-500 ring-secondary-500 ring-4'
				: 'dark:bg-surface-800 bg-white'}"
		>
			<!-- Pal Badge (smaller for list view) -->
			<div class="flex-shrink-0">
				<div class="h-16 w-16">
					<UPSPalBadge {upsPal} onSelect={handlePalSelect} />
				</div>
			</div>

			<!-- Pal Info -->
			<div class="min-w-0 flex-1">
				<div class="mb-1 flex items-center gap-2">
					<h3 class="text-surface-900 dark:text-surface-100 truncate text-base font-medium">
						{upsPal.nickname || upsPal.character_id}
					</h3>
					{#if upsPal.nickname && upsPal.nickname !== upsPal.character_id}
						<span class="text-surface-500 dark:text-surface-400 truncate text-sm">
							({upsPal.character_id})
						</span>
					{/if}
					<span class="text-surface-600 dark:text-surface-300 text-sm">
						{m.level_abbr()}. {upsPal.level}
					</span>
				</div>

				<!-- Tags -->
				{#if upsPal.tags && upsPal.tags.length > 0}
					<div class="mb-2 flex flex-wrap gap-1">
						{#each upsPal.tags.slice(0, 3) as tag}
							<span
								class="bg-primary-100 text-primary-800 dark:bg-primary-900/30 dark:text-primary-300 inline-flex items-center rounded px-2 py-0.5 text-xs font-medium"
							>
								{tag}
							</span>
						{/each}
						{#if upsPal.tags.length > 3}
							<span class="text-surface-500 text-xs">{m.and_more_count({ count: upsPal.tags.length - 3 })}</span>
						{/if}
					</div>
				{/if}

				<!-- Notes -->
				{#if upsPal.notes}
					<p class="text-surface-600 dark:text-surface-400 mb-1 line-clamp-2 text-sm">
						{upsPal.notes}
					</p>
				{/if}

				<!-- Source Info -->
				<div class="text-surface-500 dark:text-surface-400 space-y-1 text-xs">
					{#if upsPal.source_save_file}
						<div>
							{m.origin_label()} <span class="font-medium">{upsPal.source_save_file}</span>
							{#if upsPal.source_player_name}
								• {c.player}: <span class="font-medium">{upsPal.source_player_name}</span>
							{/if}
							{#if upsPal.source_storage_type}
								• {m.import_from()}: <span class="font-medium uppercase">{upsPal.source_storage_type}</span>
							{/if}
						</div>
					{/if}
					<div>
						{m.added_label()} <span class="font-medium">{formatDate(upsPal.created_at)}</span>
						{#if upsPal.updated_at !== upsPal.created_at}
							• {m.modified_label()} <span class="font-medium">{formatDate(upsPal.updated_at)}</span>
						{/if}
					</div>
				</div>
			</div>

			<!-- Stats -->
			<div class="flex-shrink-0 text-right">
				<div class="text-surface-500 dark:text-surface-400 space-y-1 text-xs">
					{#if upsPal.transfer_count > 0}
						<div title={m.transfer({ count: 2 })}>
							📤 {upsPal.transfer_count}
						</div>
					{/if}
					{#if upsPal.clone_count > 0}
						<div title={m.clones()}>
							🔄 {upsPal.clone_count}
						</div>
					{/if}
					{#if upsPal.last_accessed_at}
						<div title={m.last_accessed()}>
							👁️ {new Date(upsPal.last_accessed_at).toLocaleDateString()}
						</div>
					{/if}
				</div>
			</div>
		</div>
	{/each}
</div>

<style>
	.line-clamp-2 {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
</style>
