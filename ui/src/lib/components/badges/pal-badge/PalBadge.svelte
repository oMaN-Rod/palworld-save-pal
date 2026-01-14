<script lang="ts">
	import { PalInfoPopup } from '$components';
	import { Tooltip } from '$components/ui';
	import { type Pal, PalGender } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { cn } from '$theme';
	import { getAppState } from '$states';
	import { palsData } from '$lib/data';
	import ContextMenu from '$components/ui/context-menu/ContextMenu.svelte';
	import { Plus, ArchiveRestore, Trash, Copy, Upload } from 'lucide-svelte';
	import { assetLoader, calculateFilters } from '$utils';
	import { staticIcons } from '$types/icons';
	import { goto } from '$app/navigation';
	import * as m from '$i18n/messages';
	import { c } from '$utils/commonTranslations';

	let {
		pal = $bindable(),
		onMove: onMove,
		onAdd,
		onClone,
		onCloneToUps,
		onDelete,
		selected = $bindable([]),
		onSelect,
		showCloneToUps = true,
		disabled = false
	} = $props<{
		pal: Pal;
		onMove: () => void;
		onAdd: () => void;
		onClone: () => void;
		onCloneToUps?: () => void;
		onDelete: () => void;
		selected?: string[];
		onSelect?: (pal: Pal, event: MouseEvent) => void;
		showCloneToUps?: boolean;
		disabled?: boolean;
	}>();

	const appState = getAppState();

	const buttonClass = $derived(
		cn(
			'outline-surface-600 xl:h-18 xl:w-18 h-16 w-16 rounded-full outline outline-2 outline-offset-2',
			selected.includes(pal.instance_id) || selected.includes(pal.id?.toString() || '')
				? 'ring-4 ring-secondary-500'
				: !disabled
					? 'hover:ring-4 hover:ring-secondary-500'
					: ''
		)
	);

	const sickClass = $derived(
		pal && pal.is_sick ? 'animate-pulse ring-4 ring-red-500 rounded-full' : ''
	);

	const palData = $derived(palsData.getByKey(pal.character_key));

	const menuItems = $derived.by(() => {
		if (!pal || pal.character_id === 'None') {
			return [
				{
					label: m.add_new_pal({ pal: c.pal }),
					onClick: onAdd,
					icon: Plus
				}
			];
		}

		const items = [
			{ label: m.move_to_entity({ entity: m.party() }), onClick: onMove, icon: ArchiveRestore },
			{ label: m.clone_selected_pal({ pal: c.pal }), onClick: onClone, icon: Copy }
		];

		if (onCloneToUps && showCloneToUps) {
			items.push({ label: m.clone_to_entity({ entity: m.ups() }), onClick: onCloneToUps, icon: Upload });
		}

		items.push({ label: m.delete_entity({ entity: c.pal }), onClick: onDelete, icon: Trash });

		return items;
	});

	const genderIcon = $derived(assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${pal.gender}.webp`));
	const palIcon = $derived.by(() => {
		if (!pal) return '';
		return assetLoader.loadMenuImage(pal.character_key, palData ? palData.is_pal : false);
	});
	const palLevel = $derived(
		appState.selectedPlayer?.level! < pal.level ? appState.selectedPlayer?.level : pal.level
	);
	const levelSyncClass = $derived(
		appState.selectedPlayer?.level! < pal.level ? 'text-red-500' : ''
	);

	function handleClick(event: MouseEvent) {
		if (disabled) return;
		if (!pal || pal.character_id === 'None') {
			onAdd();
			return;
		}

		if ((event.ctrlKey || event.metaKey) && onSelect) {
			event.preventDefault();
			event.stopPropagation();
			onSelect(pal, event);
		} else {
			handlePalSelect();
		}
	}

	function handlePalSelect() {
		if (!pal || pal.character_id === 'None') return;
		appState.selectedPal = pal;
		goto('/edit/pal');
	}
</script>

<ContextMenu items={menuItems} menuClass="bg-surface-700" xOffset={-32}>
	<button class={buttonClass} onclick={handleClick}>
		{#if pal && pal.character_id !== 'None'}
			<Tooltip
				popupClass="p-4 mt-12 bg-surface-800"
				rounded="rounded-none"
				position="right"
				useArrow={false}
				{disabled}
			>
				<div class="flex flex-col">
					<div class={cn('relative flex items-center justify-center', sickClass)}>
						{#if pal.is_boss}
							<div class="absolute -left-4 -top-1 h-6 w-6 xl:h-8 xl:w-8">
								<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge" />
							</div>
						{/if}
						{#if pal.is_predator}
							<div class="absolute -left-4 -top-1 h-6 w-6 xl:h-8 xl:w-8">
								<img
									src={staticIcons.predatorIcon}
									alt="Alpha"
									class="pal-element-badge"
									style="filter: {calculateFilters('#FF0000')};"
								/>
							</div>
						{/if}
						{#if pal.is_lucky}
							<div class="absolute -left-4 -top-1 h-6 w-6 xl:h-8 xl:w-8">
								<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge" />
							</div>
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
						{#if palLevel}
							<div class="absolute -bottom-4 -left-3 h-6 w-6 xl:h-8 xl:w-8">
								<span class="text-xs {levelSyncClass} font-bold">lvl {palLevel}</span>
							</div>
						{/if}
					</div>
				</div>

				{#snippet popup()}
					<PalInfoPopup bind:pal />
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
