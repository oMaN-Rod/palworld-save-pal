<script lang="ts">
	import SectionTabs from '../SectionTabs.svelte';

	let { active = $bindable('stats') }: { active?: string } = $props();

	const tabs = [
		{ id: 'stats', label: 'Stats' },
		{ id: 'inventory', label: 'Inventory' },
		{ id: 'gear', label: 'Gear' }
	];
</script>

<SectionTabs {tabs} bind:active label="Player sections" idPrefix="harness" />
<span data-testid="active-value">{active}</span>
