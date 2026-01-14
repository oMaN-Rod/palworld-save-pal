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
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

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
			title: m.delete_selected_entity({ entity: m.preset({ count: selectedPresets.length }) }),
			message: m.delete_count_entities_confirm({
				entity: m.preset({ count: selectedPresets.length }),
				count: selectedPresets.length
			}),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});

		if (confirmed) {
			const presetIds = selectedPresets.map((preset) => preset.id);
			await presetsData.removePresetProfiles(presetIds);
			toast.add(
				m.deleted_entity({
					entity: m.preset({ count: selectedPresets.length }),
					count: selectedPresets.length
				}),
				m.success(),
				'success'
			);
			selectedPresets = [];
		}
	}

	async function handleDeletePreset(preset: ExtendedPresetProfile) {
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: m.preset({ count: 1 }) }),
			message: m.delete_entity_confirm({ entity: m.preset({ count: 1 }) }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (!confirmed) return;
		presetsData.removePresetProfiles([preset.id]);
		selectedPresets = selectedPresets.filter((p) => p.id !== preset.id);
		toast.add(
			m.deleted_entity({ entity: preset.name, count: m.preset({ count: 1 }) }),
			m.success(),
			'success'
		);
	}

	$effect(() => {
		if (searchQuery) {
			debouncedSearch();
		}
	});

	async function handleNukeAll() {
		const confirmed = await modal.showConfirmModal({
			title: m.nuke_all_presets(),
			message: m.nuke_presets_confirm(),
			confirmText: m.delete(),
			cancelText: m.cancel()
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

{#snippet tabButton(tab: PresetType, label: string)}
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
		<span class="text-xs 2xl:text-base">{label}</span>
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
			<div class="text-surface-400 text-sm">{m.no_detailed_info()}</div>
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
					{@render tabButton('pal', c.pal)}
					{@render tabButton('inventory', m.inventory())}
					{@render tabButton('passive', m.passive())}
					{@render tabButton('active', m.active())}
					{@render tabButton('storage', m.storage())}
				</nav>
			</div>

			<div class="flex items-center space-x-2">
				<div class="grow">
					<Input bind:value={searchQuery} placeholder={m.search_presets()} inputClass="w-full" />
				</div>

				<TooltipButton
					popupLabel={m.clear_entity({ entity: m.search() })}
					onclick={() => {
						searchQuery = '';
					}}
				>
					<RefreshCcw class="h-6 w-6" />
				</TooltipButton>
			</div>

			<div class="btn-group bg-surface-900 w-full items-center rounded-sm p-1">
				<TooltipButton
					popupLabel={m.import_preset()}
					onclick={handleImportPreset}
					buttonClass="hover:bg-success-500/50"
				>
					<Upload size={20} />
				</TooltipButton>

				<TooltipButton
					popupLabel={m.delete_selected_entity({
						entity: m.preset({ count: selectedPresets.length })
					})}
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
					<span class="font-bold">{m.name()}</span>
					<span class="font-bold">{m.actions()}</span>
				{/snippet}
				{#snippet listItem(preset)}
					<span class="grow">{preset.name}</span>
				{/snippet}
				{#snippet listItemActions(preset)}
					<TooltipButton
						popupLabel={m.export_preset()}
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
						<span class="text-surface-400 text-sm">{m.type_label({ type: preset.type })}</span>
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
						<img
							src={staticIcons.sadIcon}
							alt={m.no_entity_selected({ entity: m.preset({ count: 1 }) })}
							class="mb-2 h-12 w-12"
						/>
						<span class="text-lg font-bold"
							>{m.no_entity_selected({ entity: m.preset({ count: 1 }) })}</span
						>
						<span class="text-surface-400 text-sm">
							{m.preset_select_details()}
						</span>
						<Tooltip label={m.nuke_all_presets()}>
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
