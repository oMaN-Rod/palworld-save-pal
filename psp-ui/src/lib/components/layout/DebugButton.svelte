<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { getAppState, getNavigationState } from '$states';
	import { Button } from '$components/ui';

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

<Button
	variant="ghost"
	size="icon"
	class={baseClass}
	onclick={async () => {
		nav.saveAndNavigate(href);
	}}
>
	<Icon icon="tabler:bug" class={iconClass} />
</Button>
