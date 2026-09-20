<script lang="ts">
	import Tooltip from '../Tooltip.svelte';
</script>

<Tooltip label="Sort inventory" baseClass="inline-block">
	<button type="button" data-testid="tooltip-reference-a">Sort</button>
</Tooltip>

<Tooltip label="Filter inventory" baseClass="inline-block">
	<button type="button" data-testid="tooltip-reference-b">Filter</button>
</Tooltip>

<div data-testid="outside">Outside</div>
