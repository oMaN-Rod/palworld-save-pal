<script lang="ts">
	import { getNavigationState, getAppState, getSocketState } from '$states';
	import { Nav } from '@skeletonlabs/skeleton-svelte';
	import { File, Pencil } from 'lucide-svelte';

	let navigationState = getNavigationState();
	let appState = getAppState();
	let ws = getSocketState();
</script>

<Nav.Rail width="48px" bind:value={navigationState.activePage}>
	{#snippet tiles()}
		{#if appState.saveFile}
			<Nav.Tile title="Edit" id="edit" href="/edit" active="bg-secondary-500">
				<Pencil />
			</Nav.Tile>
		{/if}
		<Nav.Tile title="File" id="file" href="/file" active="bg-secondary-500">
			<File />
		</Nav.Tile>
	{/snippet}
</Nav.Rail>
