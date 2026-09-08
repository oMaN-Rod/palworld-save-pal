<script lang="ts">
	import { ItemBadge } from '$components/shared';
	import { ItemHeader, Spinner } from '$components/ui';
	import { containerSlots } from './liveView.utils';
	import type { GameInventoryContainerJson, GameInventoryJson } from '$states/gameState.svelte';
	import {
		EPalPlayerEquipItemSlotType,
		PLAYER_EQUIP_ACCESSORY_SLOTS,
		SPHERE_MODULE_SLOT,
		type ItemContainerSlot
	} from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { itemsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/tabs';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		inventory = null,
		error,
		loading = false,
		onSetSlot,
		editDisabledReason
	}: {
		inventory?: GameInventoryJson | null;
		error?: string;
		loading?: boolean;
		onSetSlot?: (
			containerId: string,
			slotIndex: number,
			staticItemId: string | null,
			count: number
		) => void;
		editDisabledReason?: string;
	} = $props();

	let group = $state('inventory');

	const containers = $derived(inventory?.containers ?? []);

	const editable = $derived(Boolean(onSetSlot) && !editDisabledReason);

	function containerOf(type: string): GameInventoryContainerJson | undefined {
		return containers.find((candidate: GameInventoryContainerJson) => candidate.type === type);
	}

	function slotsOf(type: string): ItemContainerSlot[] {
		const container = containerOf(type);
		return container ? containerSlots(container) : [];
	}

	function idOf(type: string): string | null {
		return containerOf(type)?.containerId ?? null;
	}

	function commit(type: string, slot: ItemContainerSlot): void {
		const containerId = idOf(type);
		if (!containerId || !onSetSlot) return;
		const staticItemId = slot.static_id === 'None' ? null : slot.static_id;
		onSetSlot(containerId, slot.slot_index, staticItemId, slot.count ?? 0);
	}

	function canEdit(type: string): boolean {
		return editable && idOf(type) !== null;
	}

	function equipSlot(slots: ItemContainerSlot[], ordinal: number): ItemContainerSlot {
		return slots[ordinal] ?? { slot_index: ordinal, static_id: 'None', count: 0 };
	}

	function itemIcon(staticId: string): string | undefined {
		if (!staticId || staticId === 'None') return undefined;
		const item = itemsData.getByKey(staticId);
		if (!item) return undefined;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${item.details.icon}.webp`);
	}

	const commonSlots = $derived(slotsOf('common'));
	const essentialSlots = $derived(slotsOf('essential'));
	const weaponSlots = $derived(slotsOf('weaponLoadout'));
	const foodSlots = $derived(slotsOf('foodEquip'));
	const equipSlots = $derived(slotsOf('playerEquipArmor'));

	const headGear = $derived(equipSlot(equipSlots, EPalPlayerEquipItemSlotType.Head));
	const bodyGear = $derived(equipSlot(equipSlots, EPalPlayerEquipItemSlotType.Body));
	const shieldGear = $derived(equipSlot(equipSlots, EPalPlayerEquipItemSlotType.Shield));
	const gliderGear = $derived(equipSlot(equipSlots, EPalPlayerEquipItemSlotType.Glider));
	const sphereModule = $derived(equipSlot(equipSlots, SPHERE_MODULE_SLOT));
	const accessoryGear = $derived(
		PLAYER_EQUIP_ACCESSORY_SLOTS.map((ordinal) => equipSlot(equipSlots, ordinal))
	);

	const headIcon = $derived(itemIcon(headGear.static_id));
	const bodyIcon = $derived(itemIcon(bodyGear.static_id));

	const degraded = $derived(
		containers.some((container: GameInventoryContainerJson) => container.status !== 'ok')
	);
</script>

{#if loading}
	<div class="flex flex-col items-center gap-3 py-8">
		<Spinner size="size-16" />
		<p class="text-surface-300 text-sm">{m.loading_entity({ entity: c.items })}</p>
	</div>
{:else if error}
	<p class="text-error-400 text-sm">{error}</p>
{:else if containers.length === 0}
	<p class="text-surface-400 text-sm">{m.no_entity_yet({ entity: c.items })}</p>
{:else}
	<div class="flex flex-col gap-2">
		{#if degraded}
			<p class="text-warning-400 text-xs">{m.live_inventory_partial()}</p>
		{/if}
		<div class="grid w-full grid-cols-[auto_1fr] gap-4">
			<div class="flex flex-col space-y-2">
				<Tabs
					listBorder="preset-outlined-surface-200-800"
					listClasses="btn-group preset-outlined-surface-200-800 w-full flex-col md:flex-row rounded-sm"
					value={group}
					onValueChange={(e: ValueChangeDetails) => (group = e.value)}
				>
					{#snippet list()}
						<Tabs.Control
							value="inventory"
							classes="w-full"
							base="border-none hover:bg-secondary-500/50 rounded-sm"
							labelBase="btn"
							stateActive="bg-secondary-800 text-white"
							padding="p-0"
						>
							{m.inventory()}
						</Tabs.Control>
						<Tabs.Control
							value="key_items"
							classes="w-full"
							base="border-none hover:bg-secondary-500/50 rounded-sm"
							labelBase="btn"
							stateActive="bg-secondary-800 text-white"
							padding="p-0"
						>
							{m.key_items()}
						</Tabs.Control>
					{/snippet}
					{#snippet content()}
						<Tabs.Panel value="inventory">
							<div
								id="live-inventory-panel"
								class="max-h-[500px] overflow-y-auto 2xl:max-h-[800px]"
							>
								<div class="m-1 grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-6">
									{#each commonSlots as slot (slot.slot_index)}
										<ItemBadge
											{slot}
											itemGroup="Common"
											disabled={!canEdit('common')}
											onUpdate={(updated) => commit('common', updated)}
										/>
									{/each}
								</div>
							</div>
						</Tabs.Panel>
						<Tabs.Panel value="key_items">
							<div
								id="live-key-items-panel"
								class="max-h-[500px] overflow-y-auto 2xl:max-h-[800px]"
							>
								<div class="m-1 grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-6">
									{#each essentialSlots as slot (slot.slot_index)}
										<ItemBadge
											{slot}
											itemGroup="KeyItem"
											disabled={!canEdit('essential')}
											onUpdate={(updated) => commit('essential', updated)}
										/>
									{/each}
								</div>
							</div>
						</Tabs.Panel>
					{/snippet}
				</Tabs>
			</div>
			<div class="flex min-h-0 flex-col 2xl:grid 2xl:grid-cols-[auto_1fr_auto]">
				<div class="flex flex-col space-y-2">
					<div id="live-weapon-equip" class="flex flex-col space-y-2">
						<ItemHeader text={m.weapon({ count: 1 })} />
						<div class="flex space-x-2 2xl:flex-col 2xl:space-y-2">
							{#each weaponSlots as slot (slot.slot_index)}
								<ItemBadge
									{slot}
									itemGroup="Weapon"
									disabled={!canEdit('weaponLoadout')}
									onUpdate={(updated) => commit('weaponLoadout', updated)}
								/>
							{/each}
						</div>
					</div>
					<div id="live-accessory-equip" class="flex flex-col space-y-2">
						<ItemHeader text={m.accessory()} />
						<div class="2xl:ml-2">
							<div class="flex max-h-36 max-w-36 gap-2 2xl:grid 2xl:grid-cols-2">
								{#each accessoryGear as slot (slot.slot_index)}
									<ItemBadge
										{slot}
										itemGroup="Accessory"
										disabled={!canEdit('playerEquipArmor')}
										onUpdate={(updated) => commit('playerEquipArmor', updated)}
									/>
								{/each}
							</div>
						</div>
					</div>
				</div>
				<div class="hidden flex-col items-center justify-center 2xl:flex">
					<span class="flex h-1/3 items-end">
						{#if headIcon}
							<img
								src={headIcon}
								alt={headGear.static_id}
								class="hidden 2xl:block 2xl:h-16 2xl:w-16"
							/>
						{/if}
					</span>
					<span class="h-2/3">
						{#if bodyIcon}
							<img
								src={bodyIcon}
								alt={bodyGear.static_id}
								class="hidden 2xl:block 2xl:h-64 2xl:w-64"
							/>
						{/if}
					</span>
				</div>
				<div id="live-gear-equip" class="mt-2 flex space-y-2 space-x-2 2xl:flex-col">
					<div class="flex flex-col space-y-2">
						<ItemHeader text={m.head()} />
						<ItemBadge
							slot={headGear}
							itemGroup="Head"
							disabled={!canEdit('playerEquipArmor')}
							onUpdate={(updated) => commit('playerEquipArmor', updated)}
						/>
					</div>
					<div class="flex flex-col space-y-2">
						<ItemHeader text={m.body()} />
						<ItemBadge
							slot={bodyGear}
							itemGroup="Body"
							disabled={!canEdit('playerEquipArmor')}
							onUpdate={(updated) => commit('playerEquipArmor', updated)}
						/>
					</div>
					<div class="flex flex-col space-y-2">
						<ItemHeader text={m.shield()} />
						<ItemBadge
							slot={shieldGear}
							itemGroup="Shield"
							disabled={!canEdit('playerEquipArmor')}
							onUpdate={(updated) => commit('playerEquipArmor', updated)}
						/>
					</div>
					<div class="flex flex-col space-y-2">
						<ItemHeader text={m.glider()} />
						<ItemBadge
							slot={gliderGear}
							itemGroup="Glider"
							disabled={!canEdit('playerEquipArmor')}
							onUpdate={(updated) => commit('playerEquipArmor', updated)}
						/>
					</div>
					<div class="flex flex-col space-y-2">
						<ItemHeader text={m.sphere_module()} baseClass="hidden 2xl:block" />
						<ItemHeader text={m.module()} baseClass="block 2xl:hidden" />
						<ItemBadge
							slot={sphereModule}
							itemGroup="SphereModule"
							disabled={!canEdit('playerEquipArmor')}
							onUpdate={(updated) => commit('playerEquipArmor', updated)}
						/>
					</div>
				</div>
				<div id="live-food-equip" class="col-span-3 space-y-2 2xl:mt-2 2xl:ml-12">
					<ItemHeader text={m.food()} />
					<div class="flex flex-row space-x-2">
						{#each foodSlots as slot (slot.slot_index)}
							<ItemBadge
								{slot}
								itemGroup="Food"
								disabled={!canEdit('foodEquip')}
								onUpdate={(updated) => commit('foodEquip', updated)}
							/>
						{/each}
					</div>
				</div>
			</div>
		</div>
	</div>
{/if}
