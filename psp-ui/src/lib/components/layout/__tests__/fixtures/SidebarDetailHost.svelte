<script lang="ts">
	import SidebarDetail from '../../SidebarDetail.svelte';

	let { detailActive = false }: { detailActive?: boolean } = $props();
</script>

<SidebarDetail {detailActive}>
	{#snippet sidebar()}
		<span data-testid="sidebar-content">sidebar</span>
	{/snippet}
	{#snippet detail()}
		<span data-testid="detail-content">detail</span>
	{/snippet}
</SidebarDetail>
