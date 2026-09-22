<script lang="ts">
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/tabs';

	import { ItemBadge } from '$components/shared';
	import type { ItemContainer, ItemContainerSlot } from '$types';
	import * as m from '$i18n/messages';

	interface Props {
		commonContainer: ItemContainer;
		essentialContainer: ItemContainer;
		group: 'inventory' | 'key_items';
		onUpdate: () => void;
		onCopyPaste: (event: MouseEvent, slot: ItemContainerSlot, canPaste: boolean) => void;
	}

	let {
		commonContainer,
		essentialContainer,
		group = $bindable(),
		onUpdate,
		onCopyPaste
	}: Props = $props();
</script>

<Tabs
	listBorder="preset-outlined-surface-200-800"
	listClasses="btn-group preset-outlined-surface-200-800 w-full flex-col md:flex-row rounded-sm"
	value={group}
	onValueChange={(e: ValueChangeDetails) => {
		if (e.value === 'inventory' || e.value === 'key_items') group = e.value;
	}}
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
			<div id="key-items-tab" class="w-full">
				{m.key_items()}
			</div>
		</Tabs.Control>
	{/snippet}
	{#snippet content()}
		<Tabs.Panel value="inventory">
			<div id="inventory-panel" class="max-h-[500px] overflow-y-auto 2xl:max-h-[800px]">
				<div class="m-1 grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-6">
					{#each commonContainer.slots as slot}
						<ItemBadge
							{slot}
							itemGroup="Common"
							onCopyPaste={(event) => onCopyPaste(event, slot, true)}
							{onUpdate}
						/>
					{/each}
				</div>
			</div>
		</Tabs.Panel>
		<Tabs.Panel value="key_items">
			<div id="key-items-panel" class="max-h-[500px] overflow-y-auto 2xl:max-h-[800px]">
				<div class="m-1 grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-6">
					{#each essentialContainer.slots as slot}
						<ItemBadge
							{slot}
							itemGroup="KeyItem"
							{onUpdate}
							onCopyPaste={(event) => onCopyPaste(event, slot, false)}
						/>
					{/each}
				</div>
			</div>
		</Tabs.Panel>
	{/snippet}
</Tabs>
