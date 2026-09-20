<script lang="ts">
	import Tooltip from '../Tooltip.svelte';

	let clicks = $state(0);
</script>

<Tooltip label="Sort inventory" baseClass="inline-block">
	<button type="button" data-testid="tooltip-reference" onclick={() => clicks++}>Sort</button>
</Tooltip>

<span data-testid="click-count">{clicks}</span>
