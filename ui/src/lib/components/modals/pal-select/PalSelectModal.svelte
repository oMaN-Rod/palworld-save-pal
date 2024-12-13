<script lang="ts">
	import { Card, Tooltip, Combobox, Input } from '$components/ui';
	import { type SelectOption } from '$types';
	import { Save, X } from 'lucide-svelte';
	import { palsData, elementsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';

	let { title = 'Select a Pal', closeModal } = $props<{
		title?: string;
		closeModal: (value: any) => void;
	}>();

	let selectOptions: SelectOption[] = $derived.by(() => {
		return Object.entries(palsData.pals)
			.filter(
				([_, pal]) => pal.is_pal && !pal.is_tower_boss && !pal.localized_name.includes('en_text')
			)
			.map(([code_name, pal]) => ({
				value: code_name,
				label: pal.localized_name
			}))
			.sort((a, b) => a.label.localeCompare(b.label));
	});
	let selectedPal: string = $state('');
	let nickname: string = $state('');

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? [selectedPal, nickname] : undefined);
	}
</script>

<Card class="bg-surface-500 min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<Combobox options={selectOptions} bind:value={selectedPal}>
		{#snippet selectOption(option)}
			{@const palImgName = option.label.toLowerCase().replaceAll(' ', '_')}
			{@const palImgPath = assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/pals/menu/${palImgName}_menu.png`
			)}
			{@const palData = palsData.pals[option.value]}
			<div class="flex items-center space-x-2">
				<img src={palImgPath} alt={option.label} class="h-8 w-8" />
				<span class="grow">{option.label}</span>
				{#each palData.element_types as elementType}
					{@const elementObj = elementsData.elements[elementType.toString()]}
					{@const elementIcon = assetLoader.loadImage(
						`${ASSET_DATA_PATH}/img/elements/${elementObj.icon}.png`
					)}
					<img src={elementIcon} alt={elementType} class="h-6 w-6" />
				{/each}
			</div>
		{/snippet}
	</Combobox>
	<Input label="Nickname" inputClass="grow" bind:value={nickname} />

	<div class="mt-2 flex flex-row items-center space-x-2">
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(true)}>
				<Save />
			</button>
			{#snippet popup()}
				<span>Save</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(false)}>
				<X />
			</button>
			{#snippet popup()}
				<span>Cancel</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>
