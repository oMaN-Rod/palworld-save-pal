<script lang="ts">
	import type { ItemContainer, ItemContainerSlot } from '$types';

	import GuildContainerGrid from './GuildContainerGrid.svelte';

	interface Props {
		chest: ItemContainer & { slots: ItemContainerSlot[] };
		onUpdate: () => void;
		onCopyPaste: (event: MouseEvent, slot: ItemContainerSlot) => void;
	}

	let { chest, onUpdate, onCopyPaste }: Props = $props();
</script>

<div
	id="guild-chest-content"
	class="max-h-[calc(100vh-var(--titlebar-h)-450px)] overflow-y-auto 2xl:max-h-[calc(100vh-var(--titlebar-h)-200px)]"
>
	<GuildContainerGrid
		container={chest}
		buildingKey="GuildChest"
		iconClass="ml-8 max-h-48 max-w-48 2xl:max-h-64 2xl:max-w-64"
		iconWrapperClass="ml-4 flex flex-col"
		{onUpdate}
		{onCopyPaste}
	/>
</div>
