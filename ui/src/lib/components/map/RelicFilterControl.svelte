<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { relicTypeIcon } from './styles';
	import { relicData } from '$lib/data';
	import * as m from '$i18n/messages';

	let {
		types,
		stats,
		enabled,
		showCollected = false,
		ontoggle
	}: {
		types: string[];
		stats: Record<string, { total: number; collected?: number }>;
		enabled: Record<string, boolean>;
		showCollected?: boolean;
		ontoggle: (type: string) => void;
	} = $props();

	let open = $state(false);

	const isVisible = (type: string) => enabled[type] !== false;
</script>

<div class="relative">
	<button
		type="button"
		class="bg-surface-900/95 hover:bg-surface-800 rounded-lg p-2 shadow-lg"
		title={m.relic_types()}
		aria-label={m.relic_types()}
		aria-expanded={open}
		onclick={() => (open = !open)}
	>
		<Icon icon="tabler:diamond" class="h-5 w-5" />
	</button>

	{#if open}
		<div
			class="bg-surface-900/95 absolute top-4 left-full z-20 ml-2 grid max-h-[min(70vh,520px)] w-72 grid-cols-2 gap-2 overflow-y-auto rounded-lg p-2 shadow-lg"
			role="group"
			aria-label={m.relic_types()}
		>
			{#each types as relicType (relicType)}
				{@const entry = stats[relicType]}
				<button
					class="flex items-center space-x-2 {isVisible(relicType) ? '' : 'opacity-25'}"
					onclick={() => ontoggle(relicType)}
				>
					<img
						src={relicTypeIcon(relicType)}
						alt={relicData.relics[relicType]?.localized_name ?? relicType}
						class="mr-1 h-5 w-5"
					/>
					<span class="truncate text-xs">
						{relicData.relics[relicType]?.localized_name ?? relicType}
					</span>
					<span class="text-surface-500 text-xs">
						{showCollected ? `${entry.collected ?? 0}/${entry.total}` : entry.total}
					</span>
				</button>
			{/each}
		</div>
	{/if}
</div>
