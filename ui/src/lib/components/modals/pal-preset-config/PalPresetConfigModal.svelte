<script lang="ts">
	import { Card, Input } from '$components/ui';
	import Tooltip from '$components/ui/tooltip/Tooltip.svelte';
	import { elementsData } from '$lib/data';
	import {
		palPresetNameDescriptionMap,
		type PalPresetConfig,
		type PalPresetPropertyNames
	} from '$types';
	import { ASSET_DATA_PATH } from '$types/icons';
	import { assetLoader, focusModal } from '$utils';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import { Save, X } from 'lucide-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';
	import { onMount } from 'svelte';

	const ignoreKeys = ['character_id', 'element'];

	let { config, palName, element, closeModal } = $props<{
		config: PalPresetConfig;
		palName: string;
		element: string;
		closeModal: (value: any) => void;
	}>();

	let name: string = $state('');
	let modalContainer: HTMLDivElement;

	const elementIcon = $derived.by(() => {
		const elementData = elementsData.getByKey(element);
		if (!elementData) {
			return '';
		}
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${elementData.badge_icon}.webp`);
	});

	function handleClose(value: any) {
		if (value) {
			closeModal({ name, config });
		} else {
			closeModal(null);
		}
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/3)]">
		<h3 class="h3">Preset Config</h3>

		<div class="mt-2 flex flex-col space-y-4">
			<Input inputClass="grow" bind:value={name} label="Name" />

			<div class="flex items-center space-x-2">
				{#if config.lock}
					<Input inputClass="grow" value={palName} label="Pal" disabled />
				{/if}

				{#if config.lock_element}
					<img src={elementIcon} alt={element} class="h-8 w-8" />
				{/if}
			</div>

			<div class="grid max-h-[60vh] grid-cols-3 gap-2 overflow-y-auto p-2">
				{#each Object.entries(config) as [property, _]}
					{#if !ignoreKeys.includes(property as string)}
						<div class="flex space-x-2">
							<Tooltip position="right" baseClass="flex items-center space-x-2">
								<Switch
									name={palPresetNameDescriptionMap[property as PalPresetPropertyNames].label}
									checked={config[property]}
									onCheckedChange={(mode: CheckedChangeDetails) => {
										config[property] = mode.checked;
									}}
								/>
								<span>{palPresetNameDescriptionMap[property as PalPresetPropertyNames].label}</span>
								{#snippet popup()}
									<span
										>{palPresetNameDescriptionMap[property as PalPresetPropertyNames]
											.description}</span
									>
								{/snippet}
							</Tooltip>
						</div>
					{/if}
				{/each}
			</div>

			<div class="mt-2 flex justify-end space-x-2">
				<Tooltip position="bottom">
					{#snippet children()}
						<button
							class="btn hover:bg-secondary-500 px-2"
							onclick={() => handleClose(true)}
							disabled={!name}
							data-modal-primary
						>
							<Save />
						</button>
					{/snippet}
					{#snippet popup()}
						<span>Save</span>
					{/snippet}
				</Tooltip>
				<Tooltip position="bottom">
					{#snippet children()}
						<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(false)}>
							<X />
						</button>
					{/snippet}
					{#snippet popup()}
						<span>Cancel</span>
					{/snippet}
				</Tooltip>
			</div>
		</div>
	</Card>
</div>
