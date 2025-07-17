<script lang="ts">
	import { Tooltip } from '$components/ui';
	import { type Pal } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { cn } from '$theme';
	import { getAppState, getNavigationState } from '$states';
	import { PalInfoPopup } from '$components';
	import StatusBadge from '$components/badges/status-badge/StatusBadge.svelte';
	import { palsData } from '$lib/data';
	import ContextMenu from '$components/ui/context-menu/ContextMenu.svelte';
	import { Plus, ArchiveRestore, Trash, Copy } from 'lucide-svelte';
	import { assetLoader, calculateFilters } from '$utils';
	import { staticIcons } from '$types/icons';

	let {
		pal = $bindable(),
		onMove,
		onAdd,
		onClone,
		onDelete,
		selected = $bindable(new Set()),
		onSelect
	} = $props<{
		pal: Pal;
		onMove: () => void;
		onAdd: () => void;
		onClone: () => void;
		onDelete: () => void;
		selected?: string[];
		onSelect?: (pal: Pal, event: MouseEvent) => void;
	}>();

	const appState = getAppState();
	const nav = getNavigationState();

	const cardClass = $derived(
		cn(
			'relative w-full outline outline-2 outline-surface-600',
			pal && selected.includes(pal.instance_id)
				? 'ring-4 ring-secondary-500'
				: 'hover:ring-4 hover:ring-secondary-500 outline-surface-600'
		)
	);
	const sickClass = $derived(pal && pal.is_sick ? 'animate-pulse ring-4 ring-red-500' : '');
	const palData = $derived(palsData.getPalData(pal.character_key));
	const levelSyncTxt = $derived(
		appState.selectedPlayer!.level < pal.level
			? `Level sync ${pal.level} 🡆 ${appState.selectedPlayer!.level}`
			: 'No Level Sync'
	);

	const menuItems = $derived.by(() => {
		if (!pal || pal.character_id === 'None') {
			return [
				{
					label: `Add a new Pal`,
					onClick: onAdd,
					icon: Plus
				}
			];
		}
		return [
			{ label: 'Move to Palbox', onClick: onMove, icon: ArchiveRestore },
			{ label: 'Clone Pal', onClick: onClone, icon: Copy },
			{ label: 'Delete Pal', onClick: onDelete, icon: Trash }
		];
	});

	const genderIcon = $derived(assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${pal.gender}.webp`));
	const palIcon = $derived.by(() => {
		if (!pal) return '';
		return assetLoader.loadMenuImage(pal.character_key, palData?.is_pal || false);
	});

	function handleClick(event: MouseEvent) {
		if (!pal || pal.character_id === 'None') {
			onAdd();
			return;
		}

		// If ctrl/cmd is pressed and onSelect is provided, handle selection
		if ((event.ctrlKey || event.metaKey) && onSelect) {
			event.preventDefault();
			event.stopPropagation();
			onSelect(pal, event);
		} else {
			// Otherwise handle normal pal selection
			handlePalSelect();
		}
	}

	function handlePalSelect() {
		if (!pal || pal.character_id === 'None') return;
		appState.selectedPal = pal;
		nav.activeTab = 'pal';
	}
</script>

<ContextMenu items={menuItems} menuClass="bg-surface-700" xOffset={-32}>
	<button class={cardClass} onclick={handleClick}>
		{#if pal && pal.character_id !== 'None'}
			<Tooltip
				popupClass="p-4 mt-12 bg-surface-800"
				rounded="rounded-none"
				position="right"
				useArrow={false}
			>
				<div class={cn('grid grid-cols-[1fr_auto] overflow-hidden', sickClass)}>
					<div class="ml-4 flex flex-col">
						<div class="flex space-x-2">
							<Tooltip label={levelSyncTxt}>
								<div class="flex items-end space-x-0.5">
									<span class="text-xs"> LV </span>
									<span class="text-lg font-bold">
										{pal.level < appState.selectedPlayer!.level
											? pal.level
											: appState.selectedPlayer!.level}
									</span>
								</div>
							</Tooltip>
							<span class="text-lg font-bold">{pal.name}</span>
							<div class="h-4 w-4 2xl:h-6 2xl:w-6">
								<img src={genderIcon} alt={pal.gender} />
							</div>
							{#if pal.is_boss}
								<div class="h-4 w-4 2xl:h-6 2xl:w-6">
									<img src={staticIcons.alphaIcon} alt="Alpha" />
								</div>
							{/if}
							{#if pal.is_predator}
								<div class="h-4 w-4 2xl:h-6 2xl:w-6">
									<img
										src={staticIcons.predatorIcon}
										alt="Alpha"
										class="pal-element-badge"
										style="filter: {calculateFilters('#FF0000')};"
									/>
								</div>
							{/if}
							{#if pal.is_lucky}
								<div class="h-4 w-4 2xl:h-6 2xl:w-6">✨</div>
							{/if}
						</div>
						<StatusBadge
							bind:pal
							showActions={false}
							healthHeight="h-4 2xl:h-6"
							stomachHeight="h-2"
							showStomachLabel={false}
						/>
					</div>
					<div class="flex flex-col">
						<div class={cn('relative flex items-center justify-center ')}>
							<img src={palIcon} alt={pal.name} class="h-20 w-20 2xl:h-24 2xl:w-24" />
						</div>
					</div>
				</div>

				{#snippet popup()}
					<PalInfoPopup bind:pal />
				{/snippet}
			</Tooltip>
		{:else}
			<div class="2xl:h-18 2xl:w-18 h-16 w-16"></div>
		{/if}
	</button>
</ContextMenu>
