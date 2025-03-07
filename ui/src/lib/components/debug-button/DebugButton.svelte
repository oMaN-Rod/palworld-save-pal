<script lang="ts">
	import { goto } from '$app/navigation';
	import { getAppState } from '$states';
	import { Bug } from 'lucide-svelte';

	const appState = getAppState();

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

<button
	class="btn {baseClass}"
	onclick={async () => {
		await appState.writeSave();
		goto(href);
	}}
>
	<Bug class={iconClass} />
</button>
