<script lang="ts">
	import { buildingsData } from '$lib/data';
	import type { PresetProfile } from '$types';
	import { ASSET_DATA_PATH } from '$types/icons';
	import { assetLoader } from '$utils';
	import InventoryItems from './InventoryItems.svelte';

	let { preset = $bindable() } = $props<{
		preset: PresetProfile;
	}>();

	const buildingData = buildingsData.getByKey(preset.storage_container.key);
</script>

<div class="">
	<div class="border-b-surface-600 mb-2 flex items-center border-b-2">
		<img
			src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${buildingData?.icon}.webp`)}
			alt={buildingData?.localized_name}
			class="mb-2 h-12 w-12"
		/>
		<h5 class="h5 py-4">
			{buildingData?.localized_name}
		</h5>
	</div>
	<InventoryItems bind:items={preset.storage_container.slots} />
</div>
