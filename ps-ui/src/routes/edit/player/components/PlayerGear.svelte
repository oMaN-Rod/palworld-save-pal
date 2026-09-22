<script lang="ts">
	import { ItemHeader } from '$components/ui';
	import { ItemBadge } from '$components/shared';
	import { itemsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import type { ItemContainer, ItemContainerSlot } from '$types';
	import * as m from '$i18n/messages';

	interface Props {
		weaponLoadOutContainer: ItemContainer;
		foodEquipContainer: ItemContainer;
		accessoryGear: ItemContainerSlot[];
		headGear: ItemContainerSlot;
		bodyGear: ItemContainerSlot;
		shieldGear: ItemContainerSlot;
		gliderGear: ItemContainerSlot;
		sphereModule: ItemContainerSlot;
		onUpdate: () => void;
		onCopyPaste: (event: MouseEvent, slot: ItemContainerSlot) => void;
	}

	let {
		weaponLoadOutContainer,
		foodEquipContainer,
		accessoryGear,
		headGear,
		bodyGear,
		shieldGear,
		gliderGear,
		sphereModule,
		onUpdate,
		onCopyPaste
	}: Props = $props();

	async function getItemIcon(staticId: string) {
		if (!staticId || staticId === 'None') return;
		const itemData = itemsData.getByKey(staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`);
	}
</script>

<div class="flex flex-col space-y-2">
	<div id="weapon-equip" class="flex flex-col space-y-2">
		<ItemHeader text={m.weapon({ count: 1 })} />
		<div class="flex space-x-2 2xl:flex-col 2xl:space-y-2">
			{#each weaponLoadOutContainer.slots as slot}
				<ItemBadge
					{slot}
					itemGroup="Weapon"
					onCopyPaste={(event) => onCopyPaste(event, slot)}
					{onUpdate}
				/>
			{/each}
		</div>
	</div>
	<div id="accessory-equip" class="flex flex-col space-y-2">
		<ItemHeader text={m.accessory()} />
		<div class="2xl:ml-2">
			<div class="flex max-h-36 max-w-36 gap-2 2xl:grid 2xl:grid-cols-2">
				{#each accessoryGear as slot}
					<ItemBadge
						{slot}
						itemGroup="Accessory"
						onCopyPaste={(event) => onCopyPaste(event, slot)}
						{onUpdate}
					/>
				{/each}
			</div>
		</div>
	</div>
</div>
<div class="hidden flex-col items-center justify-center 2xl:flex">
	<span class="flex h-1/3 items-end">
		{#await getItemIcon(headGear.static_id) then icon}
			{#if icon}
				<img src={icon} alt={headGear.static_id} class="hidden 2xl:block 2xl:h-16 2xl:w-16" />
			{/if}
		{/await}
	</span>
	<span class="h-2/3">
		{#await getItemIcon(bodyGear.static_id) then icon}
			{#if icon}
				<img src={icon} alt={bodyGear.static_id} class="hidden 2xl:block 2xl:h-64 2xl:w-64" />
			{/if}
		{/await}
	</span>
</div>
<div id="gear-equip" class="mt-2 flex space-y-2 space-x-2 2xl:flex-col">
	<div class="flex flex-col space-y-2">
		<ItemHeader text={m.head()} />
		<ItemBadge
			slot={headGear}
			itemGroup="Head"
			onCopyPaste={(event) => onCopyPaste(event, headGear)}
			{onUpdate}
		/>
	</div>
	<div class="flex flex-col space-y-2">
		<ItemHeader text={m.body()} />
		<ItemBadge
			slot={bodyGear}
			itemGroup="Body"
			onCopyPaste={(event) => onCopyPaste(event, bodyGear)}
			{onUpdate}
		/>
	</div>
	<div class="flex flex-col space-y-2">
		<ItemHeader text={m.shield()} />
		<ItemBadge
			slot={shieldGear}
			itemGroup="Shield"
			onCopyPaste={(event) => onCopyPaste(event, shieldGear)}
			{onUpdate}
		/>
	</div>
	<div class="flex flex-col space-y-2">
		<ItemHeader text={m.glider()} />
		<ItemBadge
			slot={gliderGear}
			itemGroup="Glider"
			onCopyPaste={(event) => onCopyPaste(event, gliderGear)}
			{onUpdate}
		/>
	</div>
	<div class="flex flex-col space-y-2">
		<ItemHeader text={m.sphere_module()} baseClass="hidden 2xl:block" />
		<ItemHeader text={m.module()} baseClass="block 2xl:hidden" />
		<ItemBadge
			slot={sphereModule}
			itemGroup="SphereModule"
			onCopyPaste={(event) => onCopyPaste(event, sphereModule)}
			{onUpdate}
		/>
	</div>
</div>
<div id="food-equip" class="col-span-3 space-y-2 2xl:mt-2 2xl:ml-12">
	<ItemHeader text={m.food()} />
	<div class="flex flex-row space-x-2">
		{#each foodEquipContainer.slots as slot}
			<ItemBadge
				{slot}
				itemGroup="Food"
				onCopyPaste={(event) => onCopyPaste(event, slot)}
				{onUpdate}
			/>
		{/each}
	</div>
</div>

<style lang="postcss">
	img {
		opacity: 0;
		animation: fadeIn 0.3s ease-in forwards;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	img:not([src]) {
		animation: fadeOut 0.3s ease-out forwards;
	}

	@keyframes fadeOut {
		from {
			opacity: 1;
		}
		to {
			opacity: 0;
		}
	}
</style>
