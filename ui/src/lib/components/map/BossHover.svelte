<script lang="ts">
	import type { Boss } from '$types';
	import BossDetails from './BossDetails.svelte';

	let {
		point
	}: {
		point: Boss & { defeated?: boolean };
	} = $props();
</script>

<div class="popup-content">
	<BossDetails {point} />
</div>

<style>
	.popup-content {
		background-color: var(--color-surface-900);
		color: white;
		padding: 8px;
		border-radius: 4px;
		min-width: 150px;
	}
</style>
