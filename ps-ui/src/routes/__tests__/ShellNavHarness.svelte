<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { NavDrawer } from '$components/layout';
	import { layout } from '$utils/layout.svelte';
	import * as m from '$i18n/messages';

	let drawerOpen = $state(false);
</script>

{#if layout.phone}
	<button
		type="button"
		class="flex size-11 items-center justify-center"
		aria-label={m.navigation()}
		onclick={() => (drawerOpen = true)}
	>
		<Icon icon="tabler:menu-2" class="size-5" />
	</button>
	<NavDrawer bind:open={drawerOpen} onClose={() => (drawerOpen = false)} />
{/if}
