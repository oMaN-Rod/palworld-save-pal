<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { navDrawer } from '$states/navDrawer.svelte';
	import * as m from '$i18n/messages';
</script>

<!-- The drawer renders nothing while closed, so this bar is the only way to open it. -->
<div
	class="border-surface-700/40 bg-surface-900/60 safe-top flex shrink-0 items-center gap-2 border-b px-2 py-1"
>
	<button
		type="button"
		class="btn p-2"
		aria-label={m.menu()}
		onclick={() => (navDrawer.open = true)}
	>
		<Icon icon="tabler:menu-2" size={20} />
	</button>
	<span class="heading-gradient text-sm font-extrabold">PalStudio</span>
</div>
