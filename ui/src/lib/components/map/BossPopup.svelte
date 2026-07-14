<script lang="ts">
	import { Card } from '$components/ui';
	import type { Boss } from '$types';
	import BossDetails from './BossDetails.svelte';

	let {
		point
	}: {
		point: Boss & { defeated?: boolean };
	} = $props();
</script>

<Card class="min-w-70 shadow-lg">
	<div class="pointer-events-auto space-y-3">
		<BossDetails {point} />
	</div>
</Card>
