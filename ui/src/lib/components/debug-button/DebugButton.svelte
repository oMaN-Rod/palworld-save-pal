<script lang="ts">
	import { goto } from '$app/navigation';
	import { Bug } from 'lucide-svelte';

	let {
		href = '',
		baseClass = '',
		iconClass = 'h-6 w-6'
	} = $props<{
		href?: string;
		baseClass?: string;
		iconClass?: string;
	}>();
</script>

<button class="btn {baseClass}" onclick={() => goto(href)}>
	<Bug class={iconClass} />
</button>
