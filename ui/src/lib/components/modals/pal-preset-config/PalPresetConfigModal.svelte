<script lang="ts">
	import { Card, Input } from '$components/ui';
	import Tooltip from '$components/ui/tooltip/Tooltip.svelte';
	import { getToastState } from '$states';
	import {
		palPresetNameDescriptionMap,
		type PalPresetConfig,
		type PalPresetPropertyNames
	} from '$types';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import { Save, X } from 'lucide-svelte';

	const toast = getToastState();

	let {
		config,
		characterId = '',
		closeModal
	} = $props<{
		config: PalPresetConfig;
		characterId: string;
		closeModal: (value: any) => void;
	}>();

	let name: string = $state('');

	function handleClose(value: any) {
		if (value) {
			closeModal({ name, config, characterId });
		} else {
			closeModal(null);
		}
	}
</script>

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">Preset Config</h3>

	<div class="mt-2 flex flex-col space-y-4">
		<Input inputClass="grow" bind:value={name} label="Name" />
		{#if config.lock}
			<Input inputClass="grow" value={characterId} label="Pal" disabled />
		{/if}

		<div class="grid max-h-[60vh] grid-cols-3 gap-2 overflow-y-auto p-2">
			{#each Object.entries(config) as [property, _]}
				{#if !property.includes('character_id')}
					<div class="flex space-x-2">
						<Tooltip position="right">
							<Switch
								name={palPresetNameDescriptionMap[property as PalPresetPropertyNames].label}
								checked={config[property]}
								onCheckedChange={(mode) => {
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
					<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(true)}>
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
