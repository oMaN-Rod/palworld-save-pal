<script lang="ts">
	import { getAppState, getNavigationState } from '$states';
	import { Bug } from 'lucide-svelte';

	const appState = getAppState();
	const nav = getNavigationState();

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
		nav.saveAndNavigate(href);
	}}
>
	<Bug class={iconClass} />
</button>
