<script lang="ts">
	import { PalCard } from '$components/pal';
	import { ContextMenu, Spinner } from '$components/ui';
	import { palsData } from '$lib/data';
	import type { GamePalJson } from '$states/gameState.svelte';
	import * as m from '$i18n/messages';
	import { deriveCharacterKey, toLivePal } from './liveView.utils';

	let {
		pal: gamePal,
		slotIndex,
		onHeal,
		onEdit,
		onAdd,
		healBusy = false,
		healing = false,
		healDisabledReason,
		editDisabledReason,
		addDisabledReason,
		levelCap
	} = $props<{
		pal?: GamePalJson;
		slotIndex?: number;
		onHeal?: (pal: GamePalJson) => void;
		onEdit?: (pal: GamePalJson) => void;
		onAdd?: (slotIndex: number) => void;
		healBusy?: boolean;
		healing?: boolean;
		healDisabledReason?: string;
		editDisabledReason?: string;
		addDisabledReason?: string;
		levelCap?: number;
	}>();

	const character_key = $derived(gamePal ? deriveCharacterKey(gamePal.characterId) : 'None');
	const palData = $derived(palsData.getByKey(character_key));
	const pal = $derived(toLivePal(gamePal, palData));
	const displayName = $derived(gamePal ? gamePal.nickname || gamePal.characterId : '');

	function handleHeal() {
		if (!gamePal || healDisabledReason || healBusy || healing) return;
		onHeal?.(gamePal);
	}

	const healLabel = $derived(
		healDisabledReason ? `${m.live_heal()} — ${healDisabledReason}` : m.live_heal()
	);

	function handleEdit() {
		if (!gamePal || editDisabledReason || healBusy) return;
		onEdit?.(gamePal);
	}

	function handleClick() {
		if (gamePal) handleEdit();
		else handleAdd();
	}

	function handleAdd() {
		if (gamePal || addDisabledReason || healBusy || slotIndex === undefined) return;
		onAdd?.(slotIndex);
	}

	const editLabel = $derived(
		editDisabledReason ? `${m.live_edit()} — ${editDisabledReason}` : m.live_edit()
	);

	const addLabel = $derived(
		addDisabledReason ? `${m.live_add_pal()} — ${addDisabledReason}` : m.live_add_pal()
	);

	const menuItems = $derived.by(() => {
		if (!gamePal) {
			if (!onAdd || slotIndex === undefined) return [];
			return [{ label: addLabel, onClick: handleAdd, icon: 'tabler:plus' }];
		}
		const items = [{ label: healLabel, onClick: handleHeal, icon: 'tabler:heart' }];
		if (onEdit) items.push({ label: editLabel, onClick: handleEdit, icon: 'tabler:edit' });
		return items;
	});
</script>

<ContextMenu items={menuItems} menuClass="bg-surface-700" xOffset={-32}>
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="relative"
		role="button"
		tabindex="0"
		aria-label={gamePal
			? m.live_edit_pal({ name: displayName })
			: onAdd && slotIndex !== undefined
				? m.live_slot_action({ action: m.live_add_pal(), slot: slotIndex + 1 })
				: undefined}
		aria-busy={healing}
		onclick={handleClick}
		onkeydown={(event: KeyboardEvent) => {
			if (event.key === 'Enter' || event.key === ' ') {
				event.preventDefault();
				handleClick();
			}
		}}
	>
		<PalCard {pal} {levelCap} disabled palIconSize="h-16 w-16 2xl:h-20 2xl:w-20" />
		{#if healing}
			<div class="bg-surface-900/70 absolute inset-0 z-10 flex items-center justify-center">
				<Spinner size="size-8" />
			</div>
		{/if}
	</div>
</ContextMenu>
