<script lang="ts">
	import { buildingsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import { ItemBadge } from '$components/shared';
	import { StoragePresets } from '$components/presets';
	import { BuildingTypeA, type ItemContainer, type ItemContainerSlot } from '$types';

	interface Props {
		container: ItemContainer & { slots: ItemContainerSlot[] };
		buildingKey?: string;
		iconClass?: string;
		iconWrapperClass?: string;
		onUpdate: () => void;
		onCopyPaste: (event: MouseEvent, slot: ItemContainerSlot) => void;
	}

	let {
		container,
		buildingKey,
		iconClass = 'max-h-48 w-full max-w-48 object-contain 2xl:max-h-64 2xl:max-w-64',
		iconWrapperClass = 'ml-2 flex flex-col',
		onUpdate,
		onCopyPaste
	}: Props = $props();

	const building = $derived(buildingsData.getByKey(buildingKey ?? container.key));

	const itemGroup = $derived(building?.type_a == BuildingTypeA.Food ? 'Food' : 'Common');

	const icon = $derived(
		building
			? assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${building.icon}.webp`)
			: staticIcons.unknownIcon
	);
</script>

<div class="flex items-start space-x-4">
	<div class="m-1 grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-6">
		{#each container.slots as slot}
			<ItemBadge {slot} {itemGroup} {onUpdate} onCopyPaste={(event) => onCopyPaste(event, slot)} />
		{/each}
	</div>
	{#if icon}
		<div class={iconWrapperClass}>
			<img src={icon} alt="Storage Container Icon" class={iconClass} />
			<StoragePresets {container} {onUpdate} />
		</div>
	{/if}
</div>
