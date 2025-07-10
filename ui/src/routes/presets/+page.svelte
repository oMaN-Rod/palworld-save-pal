<script lang="ts">
	import { presetsData } from '$lib/data';
	import { Card, List, TooltipButton, Input, Tooltip } from '$components/ui';
	import { getModalState, getToastState } from '$states';
	import { debounce } from '$utils';
	import { Trash, RefreshCcw, Download, Upload } from 'lucide-svelte';
	import { cn } from '$theme';
	import { MessageType, type PresetProfile } from '$types';
	import { staticIcons } from '$types/icons';
	import PalPreset from './components/PalPreset.svelte';
	import ActiveSkills from './components/ActiveSkills.svelte';
	import PassiveSkills from './components/PassiveSkills.svelte';
	import PlayerInventory from './components/PlayerInventory.svelte';
	import StorageInventory from './components/StorageInventory.svelte';
	import Nuke from '$components/ui/icons/Nuke.svelte';
	import { send, sendAndWait } from '$utils/websocketUtils';

	const modal = getModalState();
	const toast = getToastState();
	const debouncedSearch = debounce(() => {}, 300);

	type ExtendedPresetProfile = PresetProfile & { id: string };
	type PresetType = 'pal' | 'inventory' | 'passive' | 'active' | 'storage';

	let activeTab: PresetType = $state('pal');
	let searchQuery = $state('');
	let selectedPresets: ExtendedPresetProfile[] = $state([]);

	const presetsClass = $derived(
		// @ts-ignore
		activeTab === 'active' || activeTab === 'passive' ? 'grid grid-cols-2' : 'flex flex-col'
	);

	const palPresets = $derived(
		Object.values(presetsData.presetProfiles)
			.filter((p) => p.type === 'pal_preset')
			.map((preset) => ({
				...preset,
				id: Object.keys(presetsData.presetProfiles)[
					Object.values(presetsData.presetProfiles).findIndex((p) => p === preset)
				]
			}))
	);

	const inventoryPresets = $derived(
		Object.values(presetsData.presetProfiles)
			.filter((p) => p.type === 'inventory')
			.map((preset) => ({
				...preset,
				id: Object.keys(presetsData.presetProfiles)[
					Object.values(presetsData.presetProfiles).findIndex((p) => p === preset)
				]
			}))
	);

	const passiveSkillPresets = $derived(
		Object.values(presetsData.presetProfiles)
			.filter((p) => p.type === 'passive_skills')
			.map((preset) => ({
				...preset,
				id: Object.keys(presetsData.presetProfiles)[
					Object.values(presetsData.presetProfiles).findIndex((p) => p === preset)
				]
			}))
	);

	const activeSkillPresets = $derived(
		Object.values(presetsData.presetProfiles)
			.filter((p) => p.type === 'active_skills')
			.map((preset) => ({
				...preset,
				id: Object.keys(presetsData.presetProfiles)[
					Object.values(presetsData.presetProfiles).findIndex((p) => p === preset)
				]
			}))
	);

	const storagePresets = $derived(
		Object.values(presetsData.presetProfiles)
			.filter((p) => p.type === 'storage')
			.map((preset) => ({
				...preset,
				id: Object.keys(presetsData.presetProfiles)[
					Object.values(presetsData.presetProfiles).findIndex((p) => p === preset)
				]
			}))
	);

	const filteredPresets = $derived.by(() => {
		let presets: ExtendedPresetProfile[] = [];

		switch (activeTab) {
			case 'pal':
				presets = palPresets;
				break;
			case 'inventory':
				presets = inventoryPresets;
				break;
			case 'passive':
				presets = passiveSkillPresets;
				break;
			case 'active':
				presets = activeSkillPresets;
				break;
			case 'storage':
				presets = storagePresets;
				break;
		}

		if (searchQuery) {
			presets = presets.filter((preset) =>
				preset.name.toLowerCase().includes(searchQuery.toLowerCase())
			);
		}

		return presets;
	});

	async function handleDeletePresets() {
		if (selectedPresets.length === 0) return;

		const confirmed = await modal.showConfirmModal({
			title: `Delete Preset${selectedPresets.length > 1 ? 's' : ''}`,
			message: `Are you sure you want to delete the ${selectedPresets.length} selected preset${selectedPresets.length > 1 ? 's' : ''}?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});

		if (confirmed) {
			const presetIds = selectedPresets.map((preset) => preset.id);
			await presetsData.removePresetProfiles(presetIds);
			toast.add(
				`Deleted ${selectedPresets.length} preset${selectedPresets.length > 1 ? 's' : ''}`,
				undefined,
				'success'
			);
			selectedPresets = [];
		}
	}

	async function handleDeletePreset(preset: ExtendedPresetProfile) {
		const confirmed = await modal.showConfirmModal({
			title: `Delete Preset?`,
			message: `Are you sure you want to delete this selected preset?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (!confirmed) return;
		presetsData.removePresetProfiles([preset.id]);
		selectedPresets = selectedPresets.filter((p) => p.id !== preset.id);
		toast.add(`Deleted preset "${preset.name}"`, undefined, 'success');
	}

	$effect(() => {
		if (searchQuery) {
			debouncedSearch();
		}
	});

	async function handleNukeAll() {
		const confirmed = await modal.showConfirmModal({
			title: `Delete Preset?`,
			message: `Are you sure you want to delete all presets? This action will restore defaults & cannot be undone.`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (!confirmed) return;
		await sendAndWait(MessageType.NUKE_PRESETS);
		presetsData.reset();
	}

	async function handleExportPreset(preset: ExtendedPresetProfile) {
		await presetsData.exportPreset(preset.id, activeTab, preset.name);
	}

	async function handleImportPreset() {
		await presetsData.importPreset();
	}
</script>

{#snippet tabButton(tab: PresetType)}
	<button
		class={cn(
			'btn hover:bg-secondary-500/50 w-1/5 rounded-sm p-2 2xl:p-4',
			activeTab === tab ? 'bg-secondary-800' : ''
		)}
		onclick={() => {
			activeTab = tab;
			selectedPresets = [];
		}}
	>
		<span class="text-xs 2xl:text-base">{tab.charAt(0).toUpperCase() + tab.slice(1)}</span>
	</button>
{/snippet}

{#snippet presetContent(index: number)}
	<div class="bg-surface-900 rounded-sm p-4">
		<!-- <h3 class="h3 mb-4">{selectedPresets[index].name}</h3> -->

		{#if activeTab === 'pal' && selectedPresets[index].pal_preset}
			<PalPreset bind:preset={selectedPresets[index]} />
		{:else if activeTab === 'active' && selectedPresets[index].skills}
			<ActiveSkills bind:preset={selectedPresets[index]} />
		{:else if activeTab === 'passive' && selectedPresets[index].skills}
			<PassiveSkills bind:preset={selectedPresets[index]} />
		{:else if activeTab === 'inventory'}
			<PlayerInventory bind:preset={selectedPresets[index]} />
		{:else if activeTab === 'storage' && selectedPresets[index].storage_container}
			<StorageInventory bind:preset={selectedPresets[index]} />
		{:else}
			<div class="text-surface-400 text-sm">No detailed information available</div>
		{/if}
	</div>
{/snippet}

<div class="flex h-full flex-col">
	<div class="grid h-full w-full grid-cols-[25%_1fr]">
		<!-- Left Controls -->
		<div class="shrink-0 space-y-2 p-4">
			<div class="flex items-center">
				<nav
					class="btn-group preset-outlined-surface-200-800 w-full flex-col rounded-sm p-2 md:flex-row"
				>
					{@render tabButton('pal')}
					{@render tabButton('inventory')}
					{@render tabButton('passive')}
					{@render tabButton('active')}
					{@render tabButton('storage')}
				</nav>
			</div>

			<div class="flex items-center space-x-2">
				<div class="grow">
					<Input bind:value={searchQuery} placeholder="Search presets..." inputClass="w-full" />
				</div>

				<TooltipButton
					popupLabel="Clear Search"
					onclick={() => {
						searchQuery = '';
					}}
				>
					<RefreshCcw class="h-6 w-6" />
				</TooltipButton>
			</div>

			<div class="btn-group bg-surface-900 w-full items-center rounded-sm p-1">
				<TooltipButton
					popupLabel="Import preset from file"
					onclick={handleImportPreset}
					buttonClass="hover:bg-success-500/50"
				>
					<Upload size={20} />
				</TooltipButton>

				<TooltipButton
					popupLabel="Delete selected preset(s)"
					onclick={handleDeletePresets}
					buttonClass="hover:bg-error-500/50"
					disabled={selectedPresets.length === 0}
				>
					<Trash size={20} />
				</TooltipButton>
			</div>

			<List
				items={filteredPresets}
				listClass="h-[calc(100vh-250px)] overflow-y-auto"
				bind:selectedItems={selectedPresets}
				multiple={true}
				headerClass="grid w-full grid-cols-[auto_1fr_auto] gap-2 rounded-sm"
			>
				{#snippet listHeader()}
					<span class="font-bold">Name</span>
					<span class="font-bold">Actions</span>
				{/snippet}
				{#snippet listItem(preset)}
					<span class="grow">{preset.name}</span>
				{/snippet}
				{#snippet listItemActions(preset)}
					<TooltipButton
						popupLabel="Export preset to file"
						onclick={() => handleExportPreset(preset)}
						buttonClass="hover:bg-primary-500/25 p-2"
					>
						<Download size={16} />
					</TooltipButton>
					<button class="btn hover:bg-error-500/25 p-2" onclick={() => handleDeletePreset(preset)}>
						<Trash size={16} />
					</button>
				{/snippet}
				{#snippet listItemPopup(preset)}
					<div class="flex flex-col">
						<span class="text-lg font-bold">{preset.name}</span>
						<span class="text-surface-400 text-sm">Type: {preset.type}</span>
					</div>
				{/snippet}
			</List>
		</div>

		<!-- Right Content -->
		<div class="overflow-y-auto p-4">
			<div class="h-[calc(100vh-32px)] overflow-auto">
				{#if selectedPresets.length === 1}
					{@render presetContent(0)}
				{:else if selectedPresets.length > 1}
					<div class="{presetsClass} gap-2">
						{#each selectedPresets as _, index}
							{@render presetContent(index)}
						{/each}
					</div>
				{:else}
					<div class="flex h-full flex-col items-center justify-center">
						<img src={staticIcons.sadIcon} alt="No preset selected" class="mb-2 h-12 w-12" />
						<span class="text-lg font-bold">No preset selected</span>
						<span class="text-surface-400 text-sm">
							Select a preset from the list to view details
						</span>
						<Tooltip label="Nuke all preset">
							<button class="btn mt-2 h-48 w-48 p-2" onclick={handleNukeAll}>
								<Nuke size={120} />
							</button>
						</Tooltip>
					</div>
				{/if}
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.attribute-icon {
		width: 32px;
		height: 32px;
	}
</style>
