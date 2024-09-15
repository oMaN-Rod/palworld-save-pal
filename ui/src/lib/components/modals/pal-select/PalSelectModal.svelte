<script lang="ts">
	import { Card, Tooltip, Combobox, Input } from '$components/ui';
	import type { SelectOption } from '$types';
	import { Save, X } from 'lucide-svelte';
	import { palsData, elementsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';

	let { title = 'Select a Pal', closeModal } = $props<{
		title?: string;
		closeModal: (value: any) => void;
	}>();

	let selectOptions: SelectOption[] = $state([]);
	let selectedPal: string = $state('');
	let nickname: string = $state('');

	async function loadPalOptions() {
		const allPals = await palsData.getAllPals();
		selectOptions = allPals
			.filter(([_, pal]) => !pal.human && !pal.tower)
			.map(([code_name, pal]) => ({
				value: code_name,
				label: pal.localized_name
			}))
			.sort((a, b) => a.label.localeCompare(b.label));
	}

	async function getElementIcon(elementType: string): Promise<string | undefined> {
		const elementObj = await elementsData.searchElement(elementType);
		if (!elementObj) return undefined;
		const iconPath = `${ASSET_DATA_PATH}/img/elements/${elementObj.icon}.png`;
		return await assetLoader.loadImage(iconPath, true);
	}

	async function getPalIcon(palName: string): Promise<string | undefined> {
		const palImgName = palName.toLowerCase().replaceAll(' ', '_');
		const iconPath = `${ASSET_DATA_PATH}/img/pals/menu/${palImgName}_menu.png`;
		return await assetLoader.loadImage(iconPath, true);
	}

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? [selectedPal, nickname] : undefined);
	}

	$effect(() => {
		loadPalOptions();
	});
</script>

<Card class="bg-surface-500 min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<Combobox options={selectOptions} bind:value={selectedPal}>
		{#snippet selectOption(option)}
			<div class="flex items-center space-x-2">
				{#await getPalIcon(option.label) then icon}
					{#if icon}
						<enhanced:img src={icon} alt={option.label} class="h-8 w-8"></enhanced:img>
					{/if}
				{/await}
				<span class="grow">{option.label}</span>
				{#await palsData.getPalInfo(option.value) then palInfo}
					{#if palInfo && palInfo.type.length > 0}
						{#await getElementIcon(palInfo.type[0]) then elementIcon}
							{#if elementIcon}
								<enhanced:img src={elementIcon} alt="Element" class="h-6 w-6"></enhanced:img>
							{/if}
						{/await}
					{/if}
				{/await}
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
