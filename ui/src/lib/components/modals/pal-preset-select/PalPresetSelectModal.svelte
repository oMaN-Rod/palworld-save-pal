<script lang="ts">
	import { Card, Tooltip, Combobox } from '$components/ui';
	import { type SelectOption } from '$types';
	import { Lock, Save, X } from 'lucide-svelte';
	import { presetsData, palsData } from '$lib/data';

	let {
		title = 'Select a Pal Preset',
		selectedPals,
		closeModal
	} = $props<{
		title?: string;
		selectedPals: { character_id: string; character_key: string }[];
		closeModal: (value: any) => void;
	}>();

	let selectOptions: SelectOption[] = $derived.by(() => {
		return Object.entries(presetsData.presetProfiles)
			.filter(([_, profile]) => {
				if (profile.type !== 'pal_preset') return false;
				if (profile.pal_preset?.lock) {
					return selectedPals.every(
						(pal: { character_id: string; character_key: string }) =>
							pal.character_id === profile.pal_preset?.character_id
					);
				}
				if (profile.pal_preset?.lock_element) {
					return selectedPals.every((pal: { character_id: string; character_key: string }) => {
						const palData = palsData.getPalData(pal.character_key);
						if (!palData) return false;
						return palData.element_types[0] === profile.pal_preset?.element;
					});
				}
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

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<Combobox options={selectOptions} bind:value={selectedPreset}>
		{#snippet selectOption(option)}
			{@const presetProfile = presetsData.presetProfiles[option.value]}
			<div class="grid grid-cols-[1fr_auto_auto] items-center gap-2">
				<span>{option.label}</span>
				{#if presetProfile.pal_preset?.lock}
					{@const palCharacterKey = selectedPals.find(
						(p: { character_id: string; character_key: string }) =>
							p.character_id === presetProfile.pal_preset?.character_id
					).character_key}
					<span class="text-sm">
						{palsData.getPalData(palCharacterKey)?.localized_name ||
							presetProfile.pal_preset?.character_id}
					</span>
					<Lock class="h-4 w-4" />
				{/if}
			</div>
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
