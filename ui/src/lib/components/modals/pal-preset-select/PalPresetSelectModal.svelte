<script lang="ts">
	import { Card, Tooltip, Combobox } from '$components/ui';
	import { type SelectOption } from '$types';
	import { Save, X } from 'lucide-svelte';
	import { presetsData } from '$lib/data';

	let { title = 'Select a Pal Preset', closeModal } = $props<{
		title?: string;
		closeModal: (value: any) => void;
	}>();

	let selectOptions: SelectOption[] = $derived.by(() => {
		return Object.entries(presetsData.presetProfiles)
			.filter(([_, profile]) => {
				if (profile.type !== 'pal_preset') return false;
				return true;
			})
			.map(([id, preset]) => ({
				value: id,
				label: preset.name
			}))
			.sort((a, b) => a.label.localeCompare(b.label));
	});
	let selectedPreset: string = $state('');

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? selectedPreset : undefined);
	}
</script>

<Card class="bg-surface-500 min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<Combobox options={selectOptions} bind:value={selectedPreset}>
		{#snippet selectOption(option)}
			{option.label}
		{/snippet}
	</Combobox>

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
