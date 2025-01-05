<script lang="ts">
	import { SectionHeader, Tooltip } from '$components/ui';
	import { type Pal, PalGender } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$lib/constants';
	import { cn } from '$theme';
	import { getAppState, getNavigationState } from '$states';
	import { ActiveSkillBadge, HealthBadge, PalHeader, PassiveSkillBadge } from '$components';
	import { palsData } from '$lib/data';
	import ContextMenu from '$components/ui/context-menu/ContextMenu.svelte';
	import { Plus, ArchiveRestore, Trash, Copy } from 'lucide-svelte';
	import { assetLoader, calculateFilters } from '$utils';

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

	let cardClass = $derived(
		cn(
			'relative w-full outline outline-2 outline-offset-2 outline-surface-600',
			pal && selected.includes(pal.instance_id)
				? 'ring-4 ring-secondary-500'
				: 'hover:ring hover:ring-secondary-500 outline-surface-600'
		)
	);

	let activeSkills = $derived.by(() => {
		if (pal) {
			let skills = [...pal.active_skills];
			while (skills.length < 3) {
				skills.push('Empty');
			}
			return skills;
		} else {
			return [];
		}
	});

	let passiveSkills = $derived.by(() => {
		if (pal) {
			let skills = [...pal.passive_skills];
			while (skills.length < 4) {
				skills.push('Empty');
			}
			return skills;
		} else {
			return [];
		}
	});

	let palData = $derived(palsData.pals[pal.character_key]);

	let menuItems = $derived.by(() => {
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

	let genderIcon = $derived(
		assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/${pal.gender}.png`)
	);
	let palIcon = $derived.by(() => {
		if (!pal) return '';
		return assetLoader.loadMenuImage(pal.character_id, palData.is_pal);
	});

	function handleClick(event: MouseEvent) {
		if (!pal || pal.character_id === 'None') return;

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
				<div class="grid grid-cols-[1fr_auto] overflow-hidden">
					<div class="ml-4 flex flex-col">
						<div class="flex space-x-2">
							<div class="flex items-end space-x-0.5">
								<span class="text-xs"> LV </span>
								<span class="text-lg font-bold">{pal.level}</span>
							</div>
							<span class="text-lg font-bold">{pal.name}</span>
							<div class="h-4 w-4 xl:h-6 xl:w-6">
								<img src={genderIcon} alt={pal.gender} />
							</div>
							{#if pal.is_boss}
								<div class="h-4 w-4 xl:h-6 xl:w-6">
									<img src={staticIcons.alphaIcon} alt="Alpha" />
								</div>
							{/if}
							{#if pal.is_predator}
								<div class="h-4 w-4 xl:h-6 xl:w-6">
									<img
										src={staticIcons.predatorIcon}
										alt="Alpha"
										class="pal-element-badge"
										style="filter: {calculateFilters('#FF0000')};"
									/>
								</div>
							{/if}
							{#if pal.is_lucky}
								<div class="h-4 w-4 xl:h-6 xl:w-6">✨</div>
							{/if}
						</div>
						<HealthBadge
							bind:pal
							player={appState.selectedPlayer}
							width="w-64"
							showActions={false}
							stomachHeight="h-2"
							showStomachLabel={false}
						/>
					</div>
					<div class="flex flex-col">
						<div class={cn('relative flex items-center justify-center ')}>
							<img src={palIcon} alt={pal.name} class="xl:h-18 xl:w-18 h-16 w-16" />
						</div>
					</div>
				</div>

				{#snippet popup()}
					<div class="flex w-[450px] flex-col space-y-2">
						<PalHeader bind:pal showActions={false} />
						<HealthBadge bind:pal player={appState.selectedPlayer} />
						{#if activeSkills.length > 0}
							<SectionHeader text="Active Skills" />
							{#each activeSkills as skill}
								<ActiveSkillBadge {skill} palCharacterId={pal.character_key} />
							{/each}
						{/if}
						{#if passiveSkills.length > 0}
							<SectionHeader text="Passive Skills" />
							<div class="grid grid-cols-2 gap-2">
								{#each passiveSkills as skill}
									<PassiveSkillBadge {skill} />
								{/each}
							</div>
						{/if}
						<span class="text-justify">{palData?.description}</span>
					</div>
				{/snippet}
			</Tooltip>
		{:else}
			<div class="h-18"></div>
		{/if}
	</button>
</ContextMenu>
