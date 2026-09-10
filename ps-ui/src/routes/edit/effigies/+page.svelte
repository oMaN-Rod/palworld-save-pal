<script lang="ts">
	import { EffigiesPanel } from '$components/player';
	import { getAppState } from '$states';
	import * as m from '$i18n/messages';

	const appState = getAppState();
</script>

{#if appState.selectedPlayer}
	<div class="relative flex h-full flex-col">
		<EffigiesPanel bind:player={appState.selectedPlayer} />
	</div>
{:else}
	<div class="flex h-full w-full items-center justify-center">
		<h2 class="h2">{m.select_player_view_entity({ entity: m.edit_effigies() })}</h2>
	</div>
{/if}
