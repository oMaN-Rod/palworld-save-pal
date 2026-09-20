<script lang="ts">
	import { PalBadge } from '$components/pal';
	import { ContextMenu, Spinner } from '$components/ui';
	import { palsData } from '$lib/data/pals.svelte';
	import { deriveCharacterKey, toLivePal } from './liveView.utils';
	import type { GamePalJson } from '$states/gameState.svelte';
	import * as m from '$i18n/messages';

	let {
		pal: gamePal,
		selected = [],
		onHeal,
		onRemove,
		onMove,
		onAdd,
		onEdit,
		slotIndex,
		movePending = false,
		isMoveSource = false,
		healBusy = false,
		healing = false,
		healDisabledReason,
		editDisabledReason,
		removeDisabledReason,
		palEditDisabledReason,
		moveDisabledReason,
		addDisabledReason,
		levelCap
	} = $props<{
		pal?: GamePalJson;
		selected?: string[];
		onHeal?: (pal: GamePalJson) => void;
		onRemove?: (pal: GamePalJson) => void;
		onMove?: (slotIndex: number) => void;
		onAdd?: (slotIndex: number) => void;
		onEdit?: (pal: GamePalJson, slotIndex: number) => void;
		slotIndex?: number;
		movePending?: boolean;
		isMoveSource?: boolean;
		healBusy?: boolean;
		healing?: boolean;
		healDisabledReason?: string;
		editDisabledReason?: string;
		removeDisabledReason?: string;
		palEditDisabledReason?: string;
		moveDisabledReason?: string;
		addDisabledReason?: string;
		levelCap?: number;
	}>();

	const character_key = $derived(gamePal ? deriveCharacterKey(gamePal.characterId) : 'None');
	const palData = $derived(palsData.getByKey(character_key));
	const displayName = $derived(gamePal ? gamePal.nickname || gamePal.characterId : '');
	const pal = $derived(toLivePal(gamePal, palData));

	function handleClick() {
		if (movePending || isMoveSource) {
			handleMove();
			return;
		}
		if (gamePal) handleEdit();
		else handleAdd();
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			handleClick();
		}
	}

	function handleHeal() {
		if (!gamePal || healDisabledReason || healBusy || healing) return;
		onHeal?.(gamePal);
	}

	const healLabel = $derived(
		healDisabledReason ? `${m.live_heal()} — ${healDisabledReason}` : m.live_heal()
	);

	const editReason = $derived(editDisabledReason || palEditDisabledReason);
	const removeReason = $derived(editDisabledReason || removeDisabledReason);
	const moveReason = $derived(editDisabledReason || moveDisabledReason);
	const addReason = $derived(editDisabledReason || addDisabledReason);

	function handleEdit() {
		if (!gamePal || editReason || healBusy || slotIndex === undefined) return;
		onEdit?.(gamePal, slotIndex);
	}

	function handleRemove() {
		if (!gamePal || removeReason || healBusy) return;
		onRemove?.(gamePal);
	}

	function handleMove() {
		if (moveReason || healBusy || slotIndex === undefined) return;
		onMove?.(slotIndex);
	}

	function handleAdd() {
		if (addReason || healBusy || slotIndex === undefined) return;
		onAdd?.(slotIndex);
	}

	const labelWith = (label: string, reason: string | undefined) =>
		reason ? `${label} — ${reason}` : label;

	type MenuItem = { label: string; onClick: () => void; icon?: string };

	const moveLabel = $derived.by(() => {
		if (isMoveSource) return m.live_move_pal_cancel();
		if (movePending) return m.live_move_pal_here();
		return m.live_move_pal();
	});

	const clickLabel = $derived.by(() => {
		const action = movePending || isMoveSource ? moveLabel : gamePal ? m.live_edit() : m.live_add_pal();
		return m.live_slot_action({ action, slot: (slotIndex ?? 0) + 1 });
	});

	const menuItems = $derived.by(() => {
		const items: MenuItem[] = [];
		if (gamePal) {
			items.push({ label: healLabel, onClick: handleHeal, icon: 'tabler:heart' });
		}
		if (onMove && slotIndex !== undefined && (gamePal || movePending)) {
			items.push({
				label: labelWith(moveLabel, moveReason),
				onClick: handleMove,
				icon: isMoveSource ? 'tabler:x' : 'tabler:arrows-exchange'
			});
		}
		if (!gamePal && !movePending && onAdd && slotIndex !== undefined) {
			items.push({ label: labelWith(m.live_add_pal(), addReason), onClick: handleAdd, icon: 'tabler:plus' });
		}
		if (gamePal && onEdit && slotIndex !== undefined) {
			items.push({
				label: labelWith(m.live_edit(), editReason),
				onClick: handleEdit,
				icon: 'tabler:edit'
			});
		}
		if (gamePal && onRemove) {
			items.push({
				label: labelWith(m.live_remove_pal(), removeReason),
				onClick: handleRemove,
				icon: 'tabler:trash'
			});
		}
		return items;
	});
</script>

<ContextMenu items={menuItems} menuClass="bg-surface-700" xOffset={-32}>
	<div class="flex flex-col items-center gap-1">
		<div
			class="relative"
			onclick={handleClick}
			onkeydown={handleKeydown}
			role="button"
			tabindex="0"
			aria-label={clickLabel}
			aria-busy={healing}
		>
			<PalBadge {pal} {selected} disabled {levelCap} />
			{#if healing}
				<div
					class="bg-surface-900/70 absolute inset-0 z-10 flex items-center justify-center rounded-full"
				>
					<Spinner size="size-8" />
				</div>
			{/if}
		</div>
		<span class="min-h-4 max-w-16 truncate text-center text-xs font-medium xl:max-w-18">
			{displayName}
		</span>
	</div>
</ContextMenu>
