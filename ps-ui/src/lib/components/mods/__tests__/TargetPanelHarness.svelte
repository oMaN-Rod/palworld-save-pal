<script lang="ts">
	import TargetPanel, { type ModsTab } from '../TargetPanel.svelte';

	let { targetId }: { targetId: string } = $props();
	let tab = $state<ModsTab>('scan');
</script>

<TargetPanel {targetId} bind:tab />
<output data-testid="tab">{tab}</output>
<button onclick={() => (tab = 'backups')}>Open backups from outside</button>
