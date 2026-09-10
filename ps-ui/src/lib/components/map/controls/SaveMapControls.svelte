<script lang="ts">
	import { mapImg } from '../style/styles';
	import * as m from '$i18n/messages';

	let {
		showRelics,
		onUnlockAllFastTravel,
		onUnlockAllWatchtowers,
		onCollectAllRelics
	}: {
		showRelics: boolean;
		onUnlockAllFastTravel: () => void;
		onUnlockAllWatchtowers: () => void;
		onCollectAllRelics: () => void;
	} = $props();
</script>

<button
	type="button"
	class="bg-surface-900/95 hover:bg-surface-800 rounded-lg p-2 shadow-lg"
	title={m.unlock_all_fast_travel()}
	aria-label={m.unlock_all_fast_travel()}
	onclick={onUnlockAllFastTravel}
>
	<img src={mapImg.fastTravel} alt={m.fast_travel()} class="h-5 w-5" />
</button>
<button
	type="button"
	class="bg-surface-900/95 hover:bg-surface-800 rounded-lg p-2 shadow-lg"
	title={m.unlock_all_watchtowers()}
	aria-label={m.unlock_all_watchtowers()}
	onclick={onUnlockAllWatchtowers}
>
	<img src={mapImg.watchTower} alt={m.watchtower()} class="h-5 w-5" />
</button>
<!-- Never offer a bulk write for pins the user cannot see. -->
{#if showRelics}
	<button
		type="button"
		class="bg-surface-900/95 hover:bg-surface-800 rounded-lg p-2 shadow-lg"
		title={m.collect_all_relics()}
		aria-label={m.collect_all_relics()}
		onclick={onCollectAllRelics}
	>
		<img src={mapImg.effigy} alt={m.relics()} class="h-5 w-5" />
	</button>
{/if}
