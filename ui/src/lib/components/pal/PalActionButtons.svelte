<script lang="ts">
	import { CornerDotButton, Tooltip } from '$components/ui';
	import { type ElementType, EntryState, type Pal, PalGender } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { palsData, elementsData } from '$lib/data';
	import { cn } from '$theme';
	import { getAppState, getNavigationState, getToastState } from '$states';
	import Bug from '@lucide/svelte/icons/bug';
	import { assetLoader, editLucky, editAlpha, editAwakened, editImported } from '$utils';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';

	let {
		pal = $bindable(),
		showActions = true,
		popup = false
	}: {
		pal: Pal;
		showActions?: boolean;
		popup?: boolean;
	} = $props();

	const appState = getAppState();
	const toast = getToastState();
	const nav = getNavigationState();

	function getPalElementTypes(character_id: string): ElementType[] | undefined {
		const palData = palsData.getByKey(character_id);
		if (!palData) return undefined;
		return palData.element_types.length > 0 ? palData.element_types : undefined;
	}

	function getPalElementBadge(elementType: string): string | undefined {
		const elementObj = elementsData.getByKey(elementType);
		if (!elementObj) return undefined;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${elementObj.badge_icon}.webp`);
	}

	function handleEditGender() {
		if (!pal) return;

		let genderCycle: Record<PalGender, PalGender> = {
			[PalGender.MALE]: PalGender.FEMALE,
			[PalGender.FEMALE]: PalGender.MALE,
			[PalGender.NONE]: PalGender.MALE
		};

		const palData = palsData.getByKey(pal.character_key);
		if (palData && !palData.is_pal) {
			genderCycle[PalGender.FEMALE] = PalGender.NONE;
		}

		pal.gender = genderCycle[pal.gender] ?? PalGender.MALE;
		pal.state = EntryState.MODIFIED;
	}

	function handleEditLucky() {
		const [type, valid] = editLucky(pal);
		if (!valid) {
			toast.add(m.pal_cannot_be_trait({ type, trait: m.lucky() }), undefined, 'warning');
		}
	}

	function handleEditAlpha() {
		const [type, valid] = editAlpha(pal);
		if (!valid) {
			toast.add(m.pal_cannot_be_trait({ type, trait: m.alpha() }), undefined, 'warning');
		}
	}
</script>

<div class={cn('flex flex-wrap items-start gap-2', popup ? '2xl:flex-col' : '')}>
	<div class="flex flex-col">
		<h6 class="h6 min-w-0 grow truncate">
			{pal.nickname || pal.name}
		</h6>
		<span class="text-xs">{pal.character_id}</span>
	</div>
	<div class="flex space-x-2">
		{#if appState.settings.debug_mode && showActions}
			<Tooltip position="bottom" label={m.debug()}>
				<CornerDotButton
					onClick={() => {
						nav.saveAndNavigate(
							`/debug?guildId=${appState.selectedPlayer?.guild_id}&playerId=${appState.selectedPlayer!.uid}&palId=${appState.selectedPal!.instance_id}`
						);
					}}
					class="h-8 w-8 p-1"
				>
					<Bug />
				</CornerDotButton>
			</Tooltip>
		{/if}
		<Tooltip position="bottom" label={m.toggle_entity({ entity: m.gender() })}>
			<CornerDotButton onClick={handleEditGender} class="h-8 w-8 p-1">
				<img
					src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${pal.gender}.webp`)}
					alt={pal.gender}
				/>
			</CornerDotButton>
		</Tooltip>
		<Tooltip position="bottom" label={m.toggle_entity({ entity: m.lucky() })}>
			<CornerDotButton
				onClick={handleEditLucky}
				class={cn('h-8 w-8 p-1', pal.is_lucky && 'bg-secondary-500/25')}
				disabled={!showActions}
			>
				<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge" />
			</CornerDotButton>
		</Tooltip>
		<Tooltip position="bottom" label={m.toggle_entity({ entity: m.alpha() })}>
			<CornerDotButton
				onClick={handleEditAlpha}
				class={cn('h-8 w-8 p-1', pal.is_boss && 'bg-secondary-500/25')}
				disabled={!showActions}
			>
				<img
					src={staticIcons.alphaIcon}
					alt="Alpha"
					class="h-8 w-8"
					style="width: 24px; height: 24px;"
				/>
			</CornerDotButton>
		</Tooltip>
		<Tooltip position="bottom" label={m.toggle_entity({ entity: m.awakened() })}>
			<CornerDotButton
				onClick={() => editAwakened(pal)}
				class={cn('h-8 w-8 p-1', pal.is_awakened && 'bg-secondary-500/25')}
				disabled={!showActions}
			>
				<img src={staticIcons.awakeningIcon} alt="Awakened" class="pal-element-badge" />
			</CornerDotButton>
		</Tooltip>
		<Tooltip position="bottom" label={m.toggle_entity({ entity: m.imported() })}>
			<CornerDotButton
				onClick={() => editImported(pal)}
				class={cn('h-8 w-8 p-1', pal.is_imported && 'bg-secondary-500/25')}
				disabled={!showActions}
			>
				<img src={staticIcons.importedIcon} alt="Imported" class="pal-element-badge" />
			</CornerDotButton>
		</Tooltip>
		{#if getPalElementTypes(pal.character_key)}
			{#each getPalElementTypes(pal.character_key)! as elementType}
				{#if getPalElementBadge(elementType)}
					<img src={getPalElementBadge(elementType)} alt={elementType} class="h-8 w-8" />
				{/if}
			{/each}
		{/if}
	</div>
</div>
