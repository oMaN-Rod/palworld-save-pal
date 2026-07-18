<script lang="ts">
	import { presetsData } from '$lib/data';
	import { Button, List, TooltipButton, Input, Tooltip, Select } from '$components/ui';
	import {
		getModalState,
		getToastState,
		getConfig,
		setMode,
		setDirection,
		setCustomOrder,
		sortPresets
	} from '$states';
	import type { PresetTypeKey, PresetSortMode } from '$states';
	import { debounce, moveIds } from '$utils';
	import {
		Trash,
		RefreshCcw,
		Download,
		Upload,
		ArrowDownAZ,
		ArrowUpAZ,
		ChevronUp,
		ChevronDown
	} from 'lucide-svelte';
	import { cn } from '$theme';
	import { MessageType, type PresetProfile } from '$types';
	import { staticIcons } from '$types/icons';
	import PalPreset from './components/PalPreset.svelte';
	import ActiveSkills from './components/ActiveSkills.svelte';
	import PassiveSkills from './components/PassiveSkills.svelte';
	import PlayerInventory from './components/PlayerInventory.svelte';
	import StorageInventory from './components/StorageInventory.svelte';
	import { Nuke } from '$components/ui';
	import { sendAndWait } from '$utils/websocketUtils';
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

	const TAB_TYPE: Record<PresetType, PresetTypeKey> = {
		pal: 'pal_preset',
		inventory: 'inventory',
		passive: 'passive_skills',
		active: 'active_skills',
		storage: 'storage'
	};
	const activeTypeKey = $derived(TAB_TYPE[activeTab]);
	const activeConfig = $derived(getConfig(activeTypeKey));

	const sortOptions = $derived([
		{ value: 'name', label: m.name() },
		{ value: 'custom', label: m.sort_custom() }
	]);

	const presetsClass = $derived(
		// @ts-ignore
		activeTab === 'active' || activeTab === 'passive' ? 'grid grid-cols-2' : 'flex flex-col'
	);

	function presetsOfType(type: PresetProfile['type']): ExtendedPresetProfile[] {
		return Object.entries(presetsData.presetProfiles)
			.filter(([, preset]) => preset.type === type)
			.map(([id, preset]) => ({ ...preset, id }));
	}

	const palPresets = $derived(presetsOfType('pal_preset'));
	const inventoryPresets = $derived(presetsOfType('inventory'));
	const passiveSkillPresets = $derived(presetsOfType('passive_skills'));
	const activeSkillPresets = $derived(presetsOfType('active_skills'));
	const storagePresets = $derived(presetsOfType('storage'));

	const activePresets = $derived.by(() => {
		switch (activeTab) {
			case 'pal':
				return palPresets;
			case 'inventory':
				return inventoryPresets;
			case 'passive':
				return passiveSkillPresets;
			case 'active':
				return activeSkillPresets;
			case 'storage':
				return storagePresets;
		}
		return [];
	});

	const filteredPresets = $derived.by(() => {
		let presets = activePresets;
		if (searchQuery) {
			presets = presets.filter((preset) =>
				preset.name.toLowerCase().includes(searchQuery.toLowerCase())
			);
		}
		return sortPresets(presets, activeTypeKey);
	});

	// Reordering acts on the full (unfiltered) order; disabled while searching so
	// hidden presets keep their place. First move auto-switches the tab to custom
	// mode, seeding the custom order from what is currently displayed.
	function moveSelected(direction: 'up' | 'down') {
		if (searchQuery || selectedPresets.length === 0) return;
		const ordered = sortPresets(activePresets, activeTypeKey).map((preset) => preset.id);
		const selectedIds = new Set(selectedPresets.map((preset) => preset.id));
		const moved = moveIds(ordered, selectedIds, direction);
		setMode(activeTypeKey, 'custom');
		setCustomOrder(activeTypeKey, moved);
	}

	function handlePanelKeydown(event: KeyboardEvent) {
		if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
		const target = event.target as HTMLElement;
		if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') return;
		if (searchQuery || selectedPresets.length === 0) return;
		event.preventDefault();
		moveSelected(event.key === 'ArrowUp' ? 'up' : 'down');
	}

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

	async function handleExportSelected() {
		if (selectedPresets.length === 0) return;
		if (selectedPresets.length === 1) {
			const preset = selectedPresets[0];
			await presetsData.exportPreset(preset.id, preset.type, preset.name);
			return;
		}
		await presetsData.exportPresets(
			selectedPresets.map((preset) => ({
				preset_id: preset.id,
				preset_type: preset.type,
				preset_name: preset.name
			}))
		);
	}

	async function handleImportPreset() {
		await presetsData.importPreset();
	}
</script>

{#snippet tabButton(tab: PresetType, label: string)}
	<Button
		variant={activeTab === tab ? 'secondary' : 'ghost'}
		size="sm"
		class={cn(
			'flex-1 tracking-wider 2xl:px-4 2xl:text-sm',
			activeTab === tab ? '' : 'text-surface-400'
		)}
		onclick={() => {
			activeTab = tab;
			selectedPresets = [];
		}}
	>
		{label}
	</Button>
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

<div class="animate-fade-in flex h-full flex-col">
	<!-- Header Banner -->
	<div class="border-surface-700 flex items-center justify-start border-b p-4">
		<nav class="border-surface-600/50 bg-surface-900 flex gap-1 rounded-sm border p-1">
			{@render tabButton('pal', c.pal)}
			{@render tabButton('inventory', m.inventory())}
			{@render tabButton('passive', m.passive())}
			{@render tabButton('active', m.active())}
			{@render tabButton('storage', m.storage())}
		</nav>
	</div>

	<div
		class="grid h-full w-full grid-cols-[minmax(200px,320px)_1fr] lg:grid-cols-[280px_1fr] xl:grid-cols-[320px_1fr]"
	>
		<!-- Left Controls -->
		<div class="shrink-0 space-y-2 overflow-y-auto p-4" onkeydown={handlePanelKeydown} role="none">
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

			<div class="flex items-center space-x-2">
				<div class="grow">
					{#key activeTypeKey}
						<Select
							options={sortOptions}
							value={activeConfig.mode}
							onChange={(v) => setMode(activeTypeKey, v as PresetSortMode)}
							label={m.sort_by()}
						/>
					{/key}
				</div>

				{#if activeConfig.mode === 'name'}
					<TooltipButton
						popupLabel={activeConfig.direction === 'asc' ? m.sort_descending() : m.sort_ascending()}
						onclick={() =>
							setDirection(activeTypeKey, activeConfig.direction === 'asc' ? 'desc' : 'asc')}
					>
						{#if activeConfig.direction === 'asc'}
							<ArrowDownAZ class="h-6 w-6" />
						{:else}
							<ArrowUpAZ class="h-6 w-6" />
						{/if}
					</TooltipButton>
				{/if}
			</div>

			<div class="border-surface-700/50 bg-surface-900 flex gap-1 rounded-sm border p-1">
				<TooltipButton
					popupLabel={m.import_preset()}
					onclick={handleImportPreset}
					buttonClass="hover:bg-secondary-500/50"
				>
					<Upload size={20} />
				</TooltipButton>

				<TooltipButton
					popupLabel={m.export_selected()}
					onclick={handleExportSelected}
					buttonClass="hover:bg-primary-500/50"
					disabled={selectedPresets.length === 0}
				>
					<Download size={20} />
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

				<div class="bg-surface-700/50 mx-1 w-px self-stretch"></div>

				<TooltipButton
					popupLabel={m.move_up()}
					onclick={() => moveSelected('up')}
					buttonClass="hover:bg-secondary-500/50"
					disabled={selectedPresets.length === 0 || !!searchQuery}
				>
					<ChevronUp size={20} />
				</TooltipButton>

				<TooltipButton
					popupLabel={m.move_down()}
					onclick={() => moveSelected('down')}
					buttonClass="hover:bg-secondary-500/50"
					disabled={selectedPresets.length === 0 || !!searchQuery}
				>
					<ChevronDown size={20} />
				</TooltipButton>
			</div>

			<List
				items={filteredPresets}
				listClass="h-[calc(100vh-300px)] overflow-y-auto"
				bind:selectedItems={selectedPresets}
				multiple={true}
				headerClass="grid w-full grid-cols-[auto_1fr_auto] gap-2 rounded-sm"
				reorderable={activeConfig.mode === 'custom' && !searchQuery}
				onReorder={(ids) => setCustomOrder(activeTypeKey, ids as string[])}
			>
				{#snippet listHeader()}
					<span class="font-bold">{m.name()}</span>
					<span class="font-bold">{m.actions()}</span>
				{/snippet}
				{#snippet listItem(preset)}
					<span class="grow truncate">{preset.name}</span>
				{/snippet}
				{#snippet listItemActions(preset)}
					<TooltipButton
						popupLabel={m.export_preset()}
						onclick={() => handleExportPreset(preset)}
						buttonClass="hover:bg-primary-500/25 p-2"
					>
						<Download size={16} />
					</TooltipButton>
					<Button
						variant="ghost"
						class="hover:bg-error-500/25 p-2"
						onclick={() => handleDeletePreset(preset)}
					>
						<Trash size={16} />
					</Button>
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
							<Button variant="ghost" class="mt-2 h-48 w-48 p-2" onclick={handleNukeAll}>
								<Nuke size={120} />
							</Button>
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
