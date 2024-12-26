<script lang="ts">
	import { Tooltip } from '$components/ui';
	import { type Pal, type PalData, PalGender } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$lib/constants';
	import { cn } from '$theme';
	import { getAppState, getNavigationState } from '$states';
	import { HealthBadge } from '$components';
	import { palsData } from '$lib/data';
	import ContextMenu from '$components/ui/context-menu/ContextMenu.svelte';
	import { Plus, ArchiveRestore, Trash } from 'lucide-svelte';
	import { assetLoader } from '$utils';

	let {
		pal = $bindable(),
		onMoveToPalbox,
		onAdd,
		onDelete
	} = $props<{
		pal: Pal;
		onMoveToPalbox: () => void;
		onAdd: () => void;
		onDelete: () => void;
	}>();

	const appState = getAppState();
	const nav = getNavigationState();

	let palData = $derived.by(() => {
		if (!pal) return;
		return palsData.pals[pal.character_id];
	});

	let menuItems = $derived.by(() => {
		if (!pal || pal.character_id === 'None') {
			return [{ label: 'Add a new Pal to your party', onClick: onAdd, icon: Plus }];
		}
		return [
			{ label: 'Move to Palbox', onClick: onMoveToPalbox, icon: ArchiveRestore },
			{ label: 'Delete Pal', onClick: onDelete, icon: Trash }
		];
	});

	let genderIcon = $derived(
		assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/${pal.gender}.png`)
	);
	let palIcon = $derived.by(() => {
		if (!pal) return '';
		return assetLoader.loadMenuImage(pal.character_id);
	});

	function handlePalSelect() {
		if (!pal || pal.character_id === 'None') return;
		appState.selectedPal = pal;
		nav.activeTab = 'pal';
	}
</script>

<ContextMenu items={menuItems} menuClass="bg-surface-700" xOffset={-32}>
	<button
		class="hover:ring-secondary-500 outline-surface-600 xl:h-18 xl:w-18 h-16 w-16 rounded-full outline outline-2 outline-offset-2 hover:ring"
		onclick={handlePalSelect}
	>
		{#if pal && pal.character_id !== 'None'}
			<Tooltip popupClass="p-2 bg-surface-700">
				<div class="flex flex-col">
					<div class={cn('relative flex items-center justify-center ')}>
						{#if pal.is_boss}
							<div class="absolute -left-1 -top-1 h-6 w-6 xl:h-8 xl:w-8">
								<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge" />
							</div>
						{/if}
						{#if pal.is_lucky}
							<div class="absolute -left-1 -top-1 h-6 w-6 xl:h-8 xl:w-8">✨</div>
						{/if}
						<img src={palIcon} alt={pal.name} class="xl:h-18 xl:w-18 h-16 w-16 rounded-full" />

						<div
							class={cn(
								'absolute -right-4 -top-1 h-6 w-6 xl:h-8 xl:w-8',
								pal.gender == PalGender.MALE ? 'text-primary-300' : 'text-tertiary-300'
							)}
						>
							<img src={genderIcon} alt={pal.gender} />
						</div>
					</div>
				</div>

				{#snippet popup()}
					<div class="flex w-[450px] flex-col space-y-2">
						<span class="text-start text-2xl font-bold">{pal.nickname || pal.name}</span>
						<HealthBadge bind:pal player={appState.selectedPlayer} />
						<span>{palData?.description}</span>
					</div>
				{/snippet}
			</Tooltip>
		{:else}
			<div
				class={cn(
					'bg-surface-700 xl:h-18 xl:w-18 relative flex h-16 w-16 items-center justify-center rounded-full'
				)}
			></div>
		{/if}
	</button>
</ContextMenu>
