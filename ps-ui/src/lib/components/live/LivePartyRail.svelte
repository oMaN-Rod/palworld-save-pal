<script lang="ts">
	import { ContextMenu, Spinner, Tooltip } from '$components/ui';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import LivePartyPane from './LivePartyPane.svelte';
	import { palsData } from '$lib/data/pals.svelte';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import type { GamePalJson } from '$states/gameState.svelte';
	import { deriveCharacterKey } from './liveView.utils';

	const PARTY_SIZE = 5;

	let {
		party = [],
		partyReadable = false,
		levelCap,
		healBusy = false,
		healingPalId = null,
		healDisabledReason,
		editDisabledReason,
		expanded = $bindable(false),
		addDisabledReason,
		onHeal,
		onEdit,
		onAdd
	}: {
		party?: GamePalJson[];
		partyReadable?: boolean;
		levelCap?: number;
		healBusy?: boolean;
		healingPalId?: string | null;
		healDisabledReason?: string;
		editDisabledReason?: string;
		expanded?: boolean;
		addDisabledReason?: string;
		onHeal?: (pal: GamePalJson) => void;
		onEdit?: (pal: GamePalJson) => void;
		onAdd?: (slotIndex: number) => void;
	} = $props();

	const slots = $derived.by(() => {
		const byIndex = new Map<number, GamePalJson>();
		for (const pal of party) {
			if (!byIndex.has(pal.slotIndex)) byIndex.set(pal.slotIndex, pal);
		}
		const size = Math.max(PARTY_SIZE, ...[...byIndex.keys()].map((index) => index + 1));
		return Array.from({ length: size }, (_, index) => ({ index, pal: byIndex.get(index) }));
	});

	function iconFor(pal: GamePalJson): string {
		const key = deriveCharacterKey(pal.characterId);
		return assetLoader.loadMenuImage(key, palsData.getByKey(key)?.is_pal ?? true);
	}

	function nameFor(pal: GamePalJson): string {
		const key = deriveCharacterKey(pal.characterId);
		return palsData.getByKey(key)?.localized_name ?? pal.characterId;
	}

	function isCapped(pal: GamePalJson): boolean {
		return levelCap !== undefined && levelCap < (pal.level ?? 0);
	}

	function levelFor(pal: GamePalJson): number {
		return isCapped(pal) ? (levelCap as number) : (pal.level ?? 0);
	}

	function menuFor(pal: GamePalJson) {
		const items = [
			{
				label: healDisabledReason ? `${m.live_heal()} — ${healDisabledReason}` : m.live_heal(),
				onClick: () => {
					if (healDisabledReason || healBusy) return;
					onHeal?.(pal);
				},
				icon: 'tabler:heart'
			}
		];
		if (onEdit) {
			items.push({
				label: editDisabledReason ? `${m.live_edit()} — ${editDisabledReason}` : m.live_edit(),
				onClick: () => edit(pal),
				icon: 'tabler:edit'
			});
		}
		return items;
	}

	function edit(pal: GamePalJson) {
		if (editDisabledReason || healBusy) return;
		onEdit?.(pal);
	}

	function add(slotIndex: number) {
		if (addDisabledReason || healBusy) return;
		onAdd?.(slotIndex);
	}
</script>

{#if expanded}
	<div class="flex flex-col gap-2">
		<button
			type="button"
			aria-label={m.live_party_collapse()}
			class="text-surface-400 hover:text-surface-200 self-end p-1"
			onclick={() => (expanded = false)}
		>
			<Icon icon="tabler:chevrons-left" size={18} />
		</button>
		<LivePartyPane
			{party}
			{partyReadable}
			{levelCap}
			{healBusy}
			{healingPalId}
			{healDisabledReason}
			{editDisabledReason}
			{addDisabledReason}
			{onHeal}
			{onEdit}
			{onAdd}
		/>
	</div>
{:else}
	<div
		id="live-party-rail"
		class="bg-surface-900 flex flex-col items-center gap-2 rounded-sm px-1 py-2"
	>
		<span class="text-surface-400 text-[10px] font-semibold tracking-wider uppercase">
			{m.party()}
		</span>

		{#if !partyReadable}
			<Tooltip label={m.live_party_unavailable()}>
				<Icon icon="tabler:eye-off" size={18} class="text-surface-500" />
			</Tooltip>
		{:else}
			{#each slots as slot (slot.index)}
				{#if slot.pal}
					{@const pal = slot.pal}
					<ContextMenu items={menuFor(pal)} menuClass="bg-surface-700" xOffset={-32}>
						<button
							type="button"
							class="relative block"
							aria-label={m.live_edit_pal({ name: pal.nickname || nameFor(pal) })}
							aria-busy={healingPalId === pal.instanceId}
							onclick={() => edit(pal)}
						>
							<img
								src={iconFor(pal)}
								alt={nameFor(pal)}
								class="outline-surface-600 h-12 w-12 rounded-full outline outline-2 outline-offset-2"
							/>
							<span
								class={cn(
									'mt-1 block text-center text-[10px] font-bold',
									isCapped(pal) ? 'text-error-500' : 'text-surface-300'
								)}
							>
								{levelFor(pal)}
							</span>
							{#if healingPalId === pal.instanceId}
								<div
									class="bg-surface-900/70 absolute inset-0 z-10 flex items-center justify-center"
								>
									<Spinner size="size-6" />
								</div>
							{/if}
						</button>
					</ContextMenu>
				{:else if onAdd}
					<button
						type="button"
						class="border-surface-700 text-surface-500 hover:border-surface-500 hover:text-surface-300 flex h-12 w-12 items-center justify-center rounded-full border-2 border-dashed"
						aria-label={m.live_slot_action({ action: m.live_add_pal(), slot: slot.index + 1 })}
						title={addDisabledReason}
						onclick={() => add(slot.index)}
					>
						<Icon icon="tabler:plus" size={16} />
					</button>
				{:else}
					<div
						class="border-surface-700 text-surface-700 flex h-12 w-12 items-center justify-center rounded-full border-2 border-dashed"
						aria-hidden="true"
					>
						<Icon icon="tabler:plus" size={16} />
					</div>
				{/if}
			{/each}
		{/if}

		<button
			type="button"
			aria-label={m.live_party_expand()}
			class="text-surface-400 hover:text-surface-200 mt-1 p-1"
			onclick={() => (expanded = true)}
		>
			<Icon icon="tabler:chevrons-right" size={18} />
		</button>
	</div>
{/if}
