<script lang="ts">
	import type { Server } from '$types';
	import type { ModsTab } from '$components/mods';
	import ServerModsPanel from '../ServerModsPanel.svelte';

	const server = { id: 7 } as Server;
	let tab = $state<ModsTab>('scan');
</script>

<ServerModsPanel {server} bind:tab />
<output data-testid="tab">{tab}</output>
