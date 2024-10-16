<script lang="ts">
	import { Tooltip } from '$components/ui';
	import { type Pal, type PalData, PalGender } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import { getAppState, getNavigationState } from '$states';
	import { HealthBadge } from '$components';
	import { palsData } from '$lib/data';
	import ContextMenu from '$components/ui/context-menu/ContextMenu.svelte';
	import { Plus, ArchiveRestore, Trash } from 'lucide-svelte';

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
	let menuItems: { label: string; onClick: () => void; icon?: any }[] = $state([]);

	let palIcon: string = $state('');
	let alphaIcon: string = $state('');
	let genderIcon: string = $state('');
	let palData: PalData | undefined = $state(undefined);

	function handlePalSelect() {
		console.log('Pal selected:', pal);
		if (!pal || pal.character_id === 'None') return;
		appState.selectedPal = pal;
		nav.activeTab = 'pal';
	}

	$effect(() => {
		const loadData = async () => {
			const palImgName = pal.name.toLowerCase().replaceAll(' ', '_');
			const icon_path = `${ASSET_DATA_PATH}/img/pals/menu/${palImgName}_menu.png`;
			const icon = await assetLoader.loadImage(icon_path, true);
			palIcon = icon;

			const alphaPath = `${ASSET_DATA_PATH}/img/icons/Alpha.png`;
			alphaIcon = await assetLoader.loadImage(alphaPath);

			const genderPath = `${ASSET_DATA_PATH}/img/icons/${pal.gender === PalGender.FEMALE ? 'female' : 'male'}.svg`;
			genderIcon = await assetLoader.loadSvg(genderPath);

			palData = await palsData.getPalInfo(pal.character_id);
		};

		if (pal) {
			loadData();
		}
	});
	$effect(() => {
		if (pal && pal.character_id !== 'None') {
			menuItems = [
				{ label: 'Move to Palbox', onClick: onMoveToPalbox, icon: ArchiveRestore },
				{ label: 'Delete Pal', onClick: onDelete, icon: Trash }
			];
		} else {
			menuItems = [{ label: 'Add a new Pal to your party', onClick: onAdd, icon: Plus }];
		}
	});
</script>

<ContextMenu bind:items={menuItems} menuClass="bg-surface-700" xOffset={-32}>
	<button
		class="hover:ring-secondary-500 outline-surface-600 xl:h-18 xl:w-18 h-16 w-16 rounded-full outline outline-2 outline-offset-2 hover:ring"
		onclick={handlePalSelect}
	>
		{#if pal && pal.character_id !== 'None'}
			<Tooltip popupClass="p-2 bg-surface-700">
				<div class="flex flex-col">
					<div class={cn('relative flex items-center justify-center ')}>
						{#if pal.is_boss}
							{#if alphaIcon}
								<div class="absolute -left-1 -top-1 h-6 w-6 xl:h-8 xl:w-8">
									<enhanced:img src={alphaIcon} alt="Alpha" class="pal-element-badge"
									></enhanced:img>
								</div>
							{/if}
						{/if}
						{#if pal.is_lucky}
							<div class="absolute -left-1 -top-1 h-6 w-6 xl:h-8 xl:w-8">✨</div>
						{/if}
						{#if palIcon}
							<enhanced:img
								src={palIcon}
								alt={pal.name}
								class="xl:h-18 xl:w-18 h-16 w-16 rounded-full"
							></enhanced:img>
						{/if}
						{#if genderIcon}
							{@const color =
								pal.gender == PalGender.MALE ? 'text-primary-300' : 'text-tertiary-300'}
							<div class={cn('absolute -right-4 -top-1 h-6 w-6 xl:h-8 xl:w-8', color)}>
								{@html genderIcon}
							</div>
						{/if}
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
