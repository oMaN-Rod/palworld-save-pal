<script lang="ts">
	import { Card, Tooltip, Combobox, Input } from '$components/ui';
	import { type SelectOption } from '$types';
	import { Save, X } from 'lucide-svelte';
	import { palsData, elementsData } from '$lib/data';
	import { ASSET_DATA_PATH, staticIcons } from '$lib/constants';
	import { assetLoader } from '$utils';

	let { title = 'Select a Pal', closeModal } = $props<{
		title?: string;
		closeModal: (value: any) => void;
	}>();

	let selectOptions: SelectOption[] = $derived.by(() => {
		return Object.entries(palsData.pals)
			.filter(([_, pal]) => {
				if (!pal.localized_name || pal.localized_name === '-') return false;
				return true;
			})
			.map(([code_name, pal]) => ({
				value: code_name,
				label: formatLabel(code_name, pal.localized_name)
			}))
			.sort((a, b) => a.label.localeCompare(b.label));
	});
	let selectedPal: string = $state('');
	let nickname: string = $state('');

	function formatLabel(palId: string, palName: string) {
		if (palId.toLowerCase().includes('predator_')) {
			palName = `${palName} (Predator)`;
		}
		if (palId.toLowerCase().includes('_oilrig')) {
			palName = `${palName} (Oil Rig)`;
		}
		if (palId.toLowerCase().includes('summon_')) {
			palName = `${palName} (Summon)`;
		}
		if (palId.toLowerCase().includes('_max')) {
			palName = `${palName} (MAX)`;
		}
		if (palId.toLowerCase().includes('raid_')) {
			palName = `${palName} (Raid)`;
		}
		if (/_(\d+)$/.test(palId.toLowerCase())) {
			const match = palId.toLowerCase().match(/_(\d+)$/);
			const level = match ? match[1] : '0';
			palName = `${palName} (Lvl ${level})`;
		}
		return palName;
	}

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? [selectedPal, nickname] : undefined);
	}

	function getIconPath(option: SelectOption) {
		const palData = palsData.pals[option.value];
		if (palData && palData.is_pal) {
			return assetLoader.loadMenuImage(option.value);
		} else if (palData && !palData.is_pal) {
			return assetLoader.loadMenuImage(option.value, false);
		} else {
			return staticIcons.sadIcon;
		}
	}
</script>

<Card class="bg-surface-500 min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<Combobox options={selectOptions} bind:value={selectedPal}>
		{#snippet selectOption(option)}
			{@const palData = palsData.pals[option.value]}
			<div class="flex items-center space-x-2">
				<img src={getIconPath(option)} alt={option.label} class="h-8 w-8" />
				<div class="grow">
					<span>{option.label}</span>
					<!-- <span class="text-xs">{option.value}</span> -->
				</div>
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
