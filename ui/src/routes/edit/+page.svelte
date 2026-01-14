<script lang="ts">
	import { goto } from '$app/navigation';
	import { getAppState } from '$states';
	import { onMount } from 'svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	const appState = getAppState();

	onMount(() => {
		if (appState.selectedPlayer) {
			goto(`/edit/player`);
		}
	});
</script>

<div class="flex w-full flex-col items-center justify-center">
	<h2 class="h2">{m.select_entity_to_edit({ entity: c.player })} 🚀</h2>
</div>
