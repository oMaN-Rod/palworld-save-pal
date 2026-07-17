<script lang="ts">
	import { Input } from '$components/ui';
	import { X } from 'lucide-svelte';

	let {
		value,
		technologies,
		onchange
	}: {
		value: string[];
		technologies: string[];
		onchange: (next: string[]) => void;
	} = $props();

	let search = $state('');

	const matches = $derived.by(() => {
		const term = search.trim().toLowerCase();
		if (!term) return [];
		return technologies
			.filter((tech) => tech.toLowerCase().includes(term) && !value.includes(tech))
			.slice(0, 20);
	});

	function add(tech: string) {
		onchange([...value, tech]);
		search = '';
	}

	function remove(tech: string) {
		onchange(value.filter((t) => t !== tech));
	}
</script>

<div class="flex flex-col gap-2">
	<Input label="Search technologies" bind:value={search} placeholder="e.g. AIcore" />

	{#if matches.length > 0}
		<div class="bg-surface-900 max-h-40 overflow-y-auto rounded-sm">
			{#each matches as tech (tech)}
				<button
					type="button"
					class="hover:bg-secondary-500/25 w-full px-2 py-1 text-left text-sm"
					onclick={() => add(tech)}
				>
					{tech}
				</button>
			{/each}
		</div>
	{/if}

	<div class="flex flex-wrap gap-1">
		{#if value.length === 0}
			<span class="text-surface-400 text-xs">No technologies denied.</span>
		{/if}
		{#each value as tech (tech)}
			<span class="bg-surface-800 flex items-center gap-1 rounded-sm px-2 py-0.5 text-xs">
				{tech}
				<button type="button" aria-label={`Remove ${tech}`} onclick={() => remove(tech)}>
					<X size={12} />
				</button>
			</span>
		{/each}
	</div>
</div>
