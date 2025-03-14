<script lang="ts">
	import { activeSkillsData, elementsData } from '$lib/data';
	import type { PresetProfile } from '$types';
	import { ASSET_DATA_PATH } from '$types/icons';
	import { assetLoader } from '$utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import InventoryItems from './InventoryItems.svelte';

	let { preset = $bindable() } = $props<{
		preset: PresetProfile;
	}>();

	let elementIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const elementType of Object.keys(elementsData.elements)) {
			const elementObj = elementsData.elements[elementType];
			if (elementObj) {
				icons[elementType] = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${elementObj.badge_icon}.png`
				) as string;
			}
		}
		return icons;
	});
</script>

<div class="space-y-4">
	<h5 class="h5 border-b-surface-600 mb-2 border-b-2 py-4">Containers</h5>
	<div class="">
		<Accordion collapsible multiple>
			{#if preset.common_container && preset.common_container.length > 0}
				<Accordion.Item
					value="common_container"
					controlHover="hover:bg-secondary-500/25"
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
				>
					{#snippet control()}
						<span class="font-bold"> Common </span>
					{/snippet}
					{#snippet panel()}
						<InventoryItems bind:items={preset.common_container} />
					{/snippet}
				</Accordion.Item>
			{/if}
			{#if preset.essential_container && preset.essential_container.length > 0}
				<Accordion.Item
					value="essential_container"
					controlHover="hover:bg-secondary-500/25"
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
				>
					{#snippet control()}
						<span class="font-bold"> Key Items </span>
					{/snippet}
					{#snippet panel()}
						<InventoryItems bind:items={preset.essential_container} />
					{/snippet}
				</Accordion.Item>
			{/if}
			{#if preset.weapon_load_out_container && preset.weapon_load_out_container.length > 0}
				<Accordion.Item
					value="weapon_load_out_container"
					controlHover="hover:bg-secondary-500/25"
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
				>
					{#snippet control()}
						<span class="font-bold"> Weapons </span>
					{/snippet}
					{#snippet panel()}
						<InventoryItems bind:items={preset.weapon_load_out_container} />
					{/snippet}
				</Accordion.Item>
			{/if}
			{#if preset.player_equipment_armor_container && preset.player_equipment_armor_container.length > 0}
				<Accordion.Item
					value="player_equipment_armor_container"
					controlHover="hover:bg-secondary-500/25"
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
				>
					{#snippet control()}
						<span class="font-bold"> Armor </span>
					{/snippet}
					{#snippet panel()}
						<InventoryItems bind:items={preset.player_equipment_armor_container} />
					{/snippet}
				</Accordion.Item>
			{/if}
			{#if preset.food_equip_container && preset.food_equip_container.length > 0}
				<Accordion.Item
					value="food_equip_container"
					controlHover="hover:bg-secondary-500/25"
					controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
				>
					{#snippet control()}
						<span class="font-bold"> Food </span>
					{/snippet}
					{#snippet panel()}
						<InventoryItems bind:items={preset.food_equip_container} />
					{/snippet}
				</Accordion.Item>
			{/if}
		</Accordion>
		{#if !preset.common_container && !preset.essential_container && !preset.weapon_load_out_container && !preset.player_equipment_armor_container && !preset.food_equip_container}
			<span class="text-surface-400 text-sm">No container data</span>
		{/if}
	</div>
</div>
