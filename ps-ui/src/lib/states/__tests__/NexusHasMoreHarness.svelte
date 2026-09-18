<script lang="ts">
	import type { NexusState } from '../nexusState.svelte';

	let { state }: { state: NexusState } = $props();
</script>

<output data-testid="has-more">{state.hasMore}</output>
