<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, CornerDotButton, Tooltip, Combobox, Input } from '$components/ui';
	import { PalGender, type SelectOption } from '$types';
	import { palsData, elementsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		title = m.select_entity({ entity: c.pal }),
		showNickname = true,
		showGender = false,
		closeModal
	} = $props<{
		title?: string;
		showNickname?: boolean;
		showGender?: boolean;
		closeModal: (value: any) => void;
	}>();

	let selectOptions: SelectOption[] = $derived.by(() => {
		return Object.entries(palsData.pals)
			.filter(([_, pal]) => {
				if (!pal.localized_name || pal.localized_name === '-') return false;
				return !pal.disabled;
			})
			.sort(([codeNameA, palA], [codeNameB, palB]) => {
				const indexA = palA.pal_deck_index;
				const indexB = palB.pal_deck_index;
				const hasPositiveIndexA = indexA > 0;
				const hasPositiveIndexB = indexB > 0;

				if (hasPositiveIndexA && hasPositiveIndexB) return indexA - indexB;
				if (hasPositiveIndexA) return -1;
				if (hasPositiveIndexB) return 1;

				return formatLabel(codeNameA, palA.localized_name).localeCompare(
					formatLabel(codeNameB, palB.localized_name)
				);
			})
			.map(([code_name, pal]) => ({
				value: code_name,
				label: formatLabel(code_name, pal.localized_name)
			}));
	});
	let selectedPal: string = $state('');
	let nickname: string = $state('');
	let gender: PalGender = $state(PalGender.FEMALE);
	let modalContainer: HTMLDivElement;

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
		closeModal(confirmed ? [selectedPal, nickname, gender] : undefined);
	}

	function toggleGender() {
		gender = gender === PalGender.FEMALE ? PalGender.MALE : PalGender.FEMALE;
	}

	function getIconPath(option: SelectOption) {
		const palData = palsData.getByKey(option.value as string);
		if (palData && palData.is_pal) {
			return assetLoader.loadMenuImage(option.value as string);
		} else if (palData && !palData.is_pal) {
			return assetLoader.loadMenuImage(option.value as string, false);
		} else {
			return staticIcons.sadIcon;
		}
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/3)]">
		<h3 class="h3">{title}</h3>
		<Combobox options={selectOptions} bind:value={selectedPal}>
			{#snippet selectOption(option)}
				{@const palData = palsData.getByKey(option.value as string)}
				<div class="flex items-center space-x-2">
					{#if (palData?.pal_deck_index ?? 0) > 0}
							<span class="text-xs">#{palData?.pal_deck_index}</span>
					{:else}
						<span class="text-xs">----</span>
					{/if}
					<img src={getIconPath(option)} alt={option.label} class="h-8 w-8" />
					
					<div class="flex flex-col grow">
						<span>{option.label}</span>
						<span class="text-xs">{option.value}</span>
					</div>
					{#if palData}
						{#each palData.element_types as elementType}
							{@const elementObj = elementsData.getByKey(elementType.toString())}
							{@const elementIcon = assetLoader.loadImage(
								`${ASSET_DATA_PATH}/img/${elementObj?.icon}.webp`
							)}
							<img src={elementIcon} alt={elementType} class="h-6 w-6" />
						{/each}
					{/if}
				</div>
			{/snippet}
		</Combobox>
		{#if showNickname}
			<Input label={m.nickname()} inputClass="grow" bind:value={nickname} />
		{/if}
		{#if showGender}
			<div class="mt-2 flex items-center space-x-2">
				<span class="text-sm font-bold">{m.gender()}</span>
				<Tooltip position="bottom">
					<CornerDotButton onClick={toggleGender} class="h-8 w-8 p-1">
						<img
							src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${gender}.webp`)}
							alt={gender}
						/>
					</CornerDotButton>
					{#snippet popup()}
						<span>{m.toggle_entity({ entity: m.gender() })}</span>
					{/snippet}
				</Tooltip>
			</div>
		{/if}

		<div class="mt-2 flex flex-row items-center space-x-2">
			<Tooltip position="bottom">
				<Button variant="ghost" size="icon" onclick={() => handleClose(true)} data-modal-primary>
					<Icon icon="tabler:device-floppy" />
				</Button>
				{#snippet popup()}
					<span>{c.save}</span>
				{/snippet}
			</Tooltip>
			<Tooltip position="bottom">
				<Button variant="ghost" size="icon" onclick={() => handleClose(false)}>
					<Icon icon="tabler:x" />
				</Button>
				{#snippet popup()}
					<span>{m.cancel()}</span>
				{/snippet}
			</Tooltip>
		</div>
	</Card>
</div>
