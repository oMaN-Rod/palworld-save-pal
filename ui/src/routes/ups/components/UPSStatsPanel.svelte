<script lang="ts">
	import { BarChart3, TrendingUp, Database, Calendar } from 'lucide-svelte';
	import { getUpsState } from '$states';

	const upsState = getUpsState();

	function formatBytes(bytes: number): string {
		if (bytes === 0) return '0 B';
		const k = 1024;
		const sizes = ['B', 'KB', 'MB', 'GB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
	}

	function formatDate(dateString: string): string {
		return new Date(dateString).toLocaleString();
	}

	const stats = $derived(upsState.stats);
	const totalPals = $derived(stats?.total_pals || 0);
	const totalCollections = $derived(stats?.total_collections || 0);
	const totalTags = $derived(stats?.total_tags || 0);
	const totalTransfers = $derived(stats?.total_transfers || 0);
	const totalClones = $derived(stats?.total_clones || 0);
	const storageSize = $derived(stats?.storage_size_mb || 0);
	const lastUpdated = $derived(stats?.last_updated);
</script>

<div class="flex h-full flex-col">
	<!-- Header -->
	<div class="border-surface-300 dark:border-surface-700 border-b p-4">
		<div class="flex items-center gap-2">
			<BarChart3 class="text-primary-500 h-5 w-5" />
			<h2 class="text-lg font-semibold">Statistics</h2>
		</div>
	</div>

	<!-- Stats Content -->
	<div class="flex-1 space-y-4 overflow-auto p-4">
		{#if stats}
			<!-- Overview Stats -->
			<div class="grid grid-cols-2 gap-3">
				<!-- Total Pals -->
				<div
					class="dark:bg-surface-800 border-surface-300 dark:border-surface-700 rounded-lg border bg-white p-3"
				>
					<div class="mb-1 flex items-center justify-between">
						<span class="text-surface-600 dark:text-surface-400 text-sm">Pals</span>
						<Database class="text-surface-400 h-4 w-4" />
					</div>
					<div class="text-surface-900 dark:text-surface-100 text-2xl font-bold">
						{totalPals.toLocaleString()}
					</div>
				</div>

				<!-- Collections -->
				<div
					class="dark:bg-surface-800 border-surface-300 dark:border-surface-700 rounded-lg border bg-white p-3"
				>
					<div class="mb-1 flex items-center justify-between">
						<span class="text-surface-600 dark:text-surface-400 text-sm">Collections</span>
						<TrendingUp class="text-surface-400 h-4 w-4" />
					</div>
					<div class="text-surface-900 dark:text-surface-100 text-2xl font-bold">
						{totalCollections}
					</div>
				</div>

				<!-- Tags -->
				<div
					class="dark:bg-surface-800 border-surface-300 dark:border-surface-700 rounded-lg border bg-white p-3"
				>
					<div class="mb-1 flex items-center justify-between">
						<span class="text-surface-600 dark:text-surface-400 text-sm">Tags</span>
						<span class="text-xl">🏷️</span>
					</div>
					<div class="text-surface-900 dark:text-surface-100 text-2xl font-bold">
						{totalTags}
					</div>
				</div>

				<!-- Storage Size -->
				<div
					class="dark:bg-surface-800 border-surface-300 dark:border-surface-700 rounded-lg border bg-white p-3"
				>
					<div class="mb-1 flex items-center justify-between">
						<span class="text-surface-600 dark:text-surface-400 text-sm">Storage</span>
						<span class="text-xl">💾</span>
					</div>
					<div class="text-surface-900 dark:text-surface-100 text-lg font-bold">
						{formatBytes(storageSize * 1024 * 1024)}
					</div>
				</div>
			</div>

			<!-- Activity Stats -->
			<div
				class="dark:bg-surface-800 border-surface-300 dark:border-surface-700 rounded-lg border bg-white p-4"
			>
				<h3 class="text-surface-900 dark:text-surface-100 mb-3 text-sm font-medium">Activity</h3>
				<div class="space-y-3">
					<div class="flex items-center justify-between">
						<div class="flex items-center gap-2">
							<span class="text-xl">📤</span>
							<span class="text-surface-600 dark:text-surface-400 text-sm">Total Exports</span>
						</div>
						<span class="font-medium">{totalTransfers.toLocaleString()}</span>
					</div>
					<div class="flex items-center justify-between">
						<div class="flex items-center gap-2">
							<span class="text-xl">🔄</span>
							<span class="text-surface-600 dark:text-surface-400 text-sm">Total Clones</span>
						</div>
						<span class="font-medium">{totalClones.toLocaleString()}</span>
					</div>
				</div>
			</div>

			<!-- Most Popular -->
			{#if stats.most_popular_character_id}
				<div
					class="dark:bg-surface-800 border-surface-300 dark:border-surface-700 rounded-lg border bg-white p-4"
				>
					<h3 class="text-surface-900 dark:text-surface-100 mb-3 text-sm font-medium">
						Most Popular
					</h3>
					<div class="space-y-2">
						<div>
							<span class="text-surface-600 dark:text-surface-400 text-sm">Character:</span>
							<span class="ml-2 font-medium">{stats.most_popular_character_id}</span>
						</div>
					</div>
				</div>
			{/if}

			<!-- Distribution -->
			{#if totalPals > 0}
				<div
					class="dark:bg-surface-800 border-surface-300 dark:border-surface-700 rounded-lg border bg-white p-4"
				>
					<h3 class="text-surface-900 dark:text-surface-100 mb-3 text-sm font-medium">
						Distribution
					</h3>
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<span class="text-surface-600 dark:text-surface-400 text-sm"
								>Avg. transfers per Pal:</span
							>
							<span class="font-medium">{(totalTransfers / totalPals).toFixed(1)}</span>
						</div>
						<div class="flex items-center justify-between">
							<span class="text-surface-600 dark:text-surface-400 text-sm"
								>Avg. clones per Pal:</span
							>
							<span class="font-medium">{(totalClones / totalPals).toFixed(1)}</span>
						</div>
						{#if totalCollections > 0}
							<div class="flex items-center justify-between">
								<span class="text-surface-600 dark:text-surface-400 text-sm"
									>Avg. Pals per collection:</span
								>
								<span class="font-medium">{(totalPals / totalCollections).toFixed(1)}</span>
							</div>
						{/if}
					</div>
				</div>
			{/if}

			<!-- Last Updated -->
			{#if lastUpdated}
				<div class="bg-surface-100 dark:bg-surface-700 rounded-lg p-3">
					<div class="text-surface-600 dark:text-surface-400 flex items-center gap-2 text-xs">
						<Calendar class="h-3 w-3" />
						<span>Last updated: {formatDate(lastUpdated)}</span>
					</div>
				</div>
			{/if}
		{:else}
			<!-- Loading State -->
			<div class="flex h-32 items-center justify-center">
				<div class="border-primary-500 h-8 w-8 animate-spin rounded-full border-b-2"></div>
			</div>
		{/if}
	</div>

	<!-- Footer Actions -->
	<div class="border-surface-300 dark:border-surface-700 border-t p-4">
		<button
			onclick={() => upsState.loadStats()}
			class="bg-primary-500 hover:bg-primary-600 w-full rounded-md px-3 py-2 text-sm text-white transition-colors"
		>
			Refresh Stats
		</button>
	</div>
</div>
