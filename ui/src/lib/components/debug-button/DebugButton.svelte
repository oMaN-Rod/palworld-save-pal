<script lang="ts">
	import { goto } from '$app/navigation';
	import { Bug } from 'lucide-svelte';

	let { href = '' } = $props<{
		href?: string;
	}>();
</script>

<button class="btn" onclick={() => goto(href)}>
	<Bug class="h-6 w-6" />
</button>
