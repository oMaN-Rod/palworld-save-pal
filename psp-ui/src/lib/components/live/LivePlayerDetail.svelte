<script lang="ts">
	import { Button, Input, SectionHeader, Spinner, Tooltip } from '$components/ui';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { PalGrid } from '$components/pal';
	import LivePalBadge from './LivePalBadge.svelte';
	import LivePalboxPager from './LivePalboxPager.svelte';
	import LiveLoadoutPane from './LiveLoadoutPane.svelte';
	import LiveGuildPane from './LiveGuildPane.svelte';
	import type {
		GameBasePalsJson,
		GameGuildContainersJson,
		GameGuildJson,
		GameGuildsJson,
		GameInventoryJson,
		GamePalJson,
		GamePlayerJson,
		GameRefusalJson
	} from '$states/gameState.svelte';
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/tabs';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { MAX_LEVEL, MIN_LEVEL } from './liveView.utils';

	let {
		player,
		pals = [],
		palsError,
		palsPage = 0,
		palsPageCount = 0,
		palsSlotCount = 0,
		palsSlotBase = 0,
		inventory = null,
		inventoryError,
		loading = false,
		palsLoading = false,
		busy = false,
		healReason,
		setLevelReason,
		setItemSlotReason,
		healAllBusy = false,
		guild = null,
		guildError = null,
		basePals = null,
		basePalsError = null,
		basePalsLoading = false,
		basePage = 0,
		onBasePageChange,
		guildContainers = null,
		guildContainersError = null,
		guildContainersLoading = false,
		guildLevelBusy = false,
		setGuildLevelReason,
		onSetBaseCampLevel,
		guildRoleBusy = false,
		setGuildRoleReason,
		onSetMemberRole,
		guilds = null,
		selectedGuildId = null,
		onSelectGuild,
		onEditBasePal,
		onAddBasePal,
		basePalEditReason,
		basePalAddReason,
		tab = $bindable('loadout'),
		healingPalId = null,
		levelBusy = false,
		onHeal,
		onHealAll,
		onRemovePal,
		onMovePal,
		onAddPal,
		onEditPal,
		palEditReason,
		movePendingSlot = null,
		removePalReason,
		movePalReason,
		addPalReason,
		onPalsPageChange,
		onSetLevel,
		onSetItemSlot
	}: {
		player: GamePlayerJson;
		pals?: GamePalJson[];
		palsError?: string;
		palsPage?: number;
		palsPageCount?: number;
		palsSlotCount?: number;
		palsSlotBase?: number;
		inventory?: GameInventoryJson | null;
		inventoryError?: string;
		loading?: boolean;
		palsLoading?: boolean;
		busy?: boolean;
		healReason?: string;
		setLevelReason?: string;
		setItemSlotReason?: string;
		healAllBusy?: boolean;
		guild?: GameGuildJson | null;
		guildError?: GameRefusalJson | null;
		basePals?: GameBasePalsJson | null;
		basePalsError?: GameRefusalJson | null;
		basePalsLoading?: boolean;
		basePage?: number;
		onBasePageChange?: (page: number) => void;
		guildContainers?: GameGuildContainersJson | null;
		guildContainersError?: GameRefusalJson | null;
		guildContainersLoading?: boolean;
		guildLevelBusy?: boolean;
		setGuildLevelReason?: string;
		onSetBaseCampLevel?: (level: number) => void;
		guildRoleBusy?: boolean;
		setGuildRoleReason?: string;
		onSetMemberRole?: (memberUid: string, role: string) => void;
		guilds?: GameGuildsJson | null;
		selectedGuildId?: string | null;
		onSelectGuild?: (guildId: string) => void;
		onEditBasePal?: (pal: GamePalJson, slotIndex: number) => void;
		onAddBasePal?: (slotIndex: number) => void;
		basePalEditReason?: string;
		basePalAddReason?: string;
		tab?: string;
		healingPalId?: string | null;
		levelBusy?: boolean;
		onHeal: (pal: GamePalJson) => void;
		onHealAll: () => void;
		onRemovePal?: (pal: GamePalJson) => void;
		onMovePal?: (slotIndex: number) => void;
		onAddPal?: (slotIndex: number) => void;
		onEditPal?: (pal: GamePalJson, slotIndex: number) => void;
		palEditReason?: string;
		movePendingSlot?: number | null;
		removePalReason?: string;
		movePalReason?: string;
		addPalReason?: string;
		onPalsPageChange: (page: number) => void;
		onSetLevel: (level: number) => Promise<boolean>;
		onSetItemSlot: (
			containerId: string,
			slotIndex: number,
			staticItemId: string | null,
			count: number
		) => void;
	} = $props();

	const boxSlots = $derived.by(() => {
		const byPosition = new Map<number, GamePalJson>();
		for (const pal of pals) {
			const position = pal.slotIndex - palsSlotBase;
			if (position >= 0 && !byPosition.has(position)) byPosition.set(position, pal);
		}
		const size = Math.max(palsSlotCount, ...[...byPosition.keys()].map((p) => p + 1), 0);
		return Array.from({ length: size }, (_, position) => ({
			position,
			pal: byPosition.get(position)
		}));
	});

	let levelDraft = $state(player.level ?? 1);
	let lastKnownLevel = player.level ?? 1;
	let levelMessage = $state<string | null>(null);

	$effect(() => {
		const current = player.level ?? 1;
		if (current === lastKnownLevel) return;
		lastKnownLevel = current;
		levelDraft = current;
	});

	async function submitSetLevel() {
		if (setLevelReason || busy) return;
		if (!Number.isInteger(levelDraft) || levelDraft < MIN_LEVEL || levelDraft > MAX_LEVEL) {
			levelMessage = m.live_level_out_of_range({ min: MIN_LEVEL, max: MAX_LEVEL });
			return;
		}
		levelMessage = null;
		await onSetLevel(levelDraft);
	}
</script>

<div class="flex flex-col gap-6">
	<Tabs value={tab} onValueChange={(e: ValueChangeDetails) => (tab = e.value)}>
		{#snippet list()}
			<Tabs.Control value="loadout" classes="justify-center w-full" padding="p-0">
				{m.loadout()}
			</Tabs.Control>
			<Tabs.Control value="palbox" classes="justify-center w-full" padding="p-0">
				{m.palbox()}
			</Tabs.Control>
			<Tabs.Control value="guild" classes="justify-center w-full" padding="p-0">
				{m.guild({ count: 1 })}
			</Tabs.Control>
		{/snippet}
		{#snippet content()}
			<Tabs.Panel value="loadout">
				<div id="live-loadout-panel" class="grid w-full grid-cols-[minmax(0,1fr)_auto] gap-4">
					<div class="min-w-0 overflow-x-auto">
						<LiveLoadoutPane
							{inventory}
							error={inventoryError}
							{loading}
							editDisabledReason={setItemSlotReason ||
								(busy ? m.live_write_in_flight() : undefined)}
							onSetSlot={onSetItemSlot}
						/>
					</div>
					<section class="flex flex-col gap-2">
						<SectionHeader text={m.actions()} />
						<div class="flex flex-wrap items-end gap-2">
							<Input
								type="number"
								label={m.level()}
								min={MIN_LEVEL}
								max={MAX_LEVEL}
								value={levelDraft}
								onValueChange={(level: number) => (levelDraft = level)}
								disabled={Boolean(setLevelReason) || busy}
							/>
							<Button
								variant="secondary"
								disabled={Boolean(setLevelReason) || busy}
								loading={levelBusy}
								title={setLevelReason}
								onclick={submitSetLevel}
							>
								{m.live_set_level()}
							</Button>
						</div>
						{#if levelMessage}
							<p class="text-error-400 text-xs">{levelMessage}</p>
						{/if}
					</section>
				</div>
			</Tabs.Panel>
			<Tabs.Panel value="palbox">
				<div id="live-palbox-panel" class="flex flex-col gap-2">
					<LivePalboxPager
						page={palsPage}
						pageCount={palsPageCount}
						disabled={loading || palsLoading}
						onPageChange={onPalsPageChange}
					/>
					{#if loading || palsLoading}
						<div class="flex flex-col items-center gap-3 py-8">
							<Spinner size="size-16" />
							<p class="text-surface-300 text-sm">{m.loading_entity({ entity: c.pals })}</p>
						</div>
					{:else if palsError}
						<p class="text-error-400 text-sm">{palsError}</p>
					{:else if boxSlots.length === 0}
						<p class="text-surface-400 text-sm">{m.no_entity_yet({ entity: c.pals })}</p>
					{:else}
						<div class="flex">
							<nav
								id="live-pal-actions"
								class="btn-group preset-outlined-surface-200-800 mr-2 flex-col items-center self-start rounded-sm"
							>
								<Tooltip label={healReason ?? m.live_heal_all()}>
									<Button
										variant="ghost"
										size="icon"
										aria-label={m.live_heal_all()}
										disabled={Boolean(healReason) || busy || pals.length === 0}
										loading={healAllBusy}
										onclick={onHealAll}
									>
										<Icon icon="tabler:heart-plus" class="h-6 w-6" />
									</Button>
								</Tooltip>
							</nav>
							<PalGrid id="live-palbox-grid" class="grow">
								{#each boxSlots as slot (slot.position)}
									<LivePalBadge
										pal={slot.pal}
										slotIndex={palsSlotBase + slot.position}
										levelCap={player.level ?? 0}
										healBusy={busy}
										healing={slot.pal !== undefined && healingPalId === slot.pal.instanceId}
										healDisabledReason={healReason}
										editDisabledReason={busy ? m.live_write_in_flight() : undefined}
										removeDisabledReason={removePalReason}
										moveDisabledReason={movePalReason}
										addDisabledReason={addPalReason}
										movePending={movePendingSlot !== null}
										isMoveSource={movePendingSlot === palsSlotBase + slot.position}
										{onHeal}
										onRemove={onRemovePal}
										onMove={onMovePal}
										onAdd={onAddPal}
										onEdit={onEditPal}
										palEditDisabledReason={palEditReason}
									/>
								{/each}
							</PalGrid>
						</div>
					{/if}
				</div>
			</Tabs.Panel>
			<Tabs.Panel value="guild">
				<LiveGuildPane
					{guild}
					error={guildError}
					{loading}
					{basePals}
					{basePalsError}
					{basePalsLoading}
					{basePage}
					{onBasePageChange}
					{guildContainers}
					{guildContainersError}
					{guildContainersLoading}
					levelBusy={guildLevelBusy}
					setLevelReason={setGuildLevelReason}
					{onSetBaseCampLevel}
					roleBusy={guildRoleBusy}
					setRoleReason={setGuildRoleReason}
					{onSetMemberRole}
					{guilds}
					{selectedGuildId}
					{onSelectGuild}
					setItemSlotReason={setItemSlotReason || (busy ? m.live_write_in_flight() : undefined)}
					onSetItemSlot={onSetItemSlot}
					{busy}
					{onEditBasePal}
					{onAddBasePal}
					basePalEditReason={basePalEditReason || (busy ? m.live_write_in_flight() : undefined)}
					basePalAddReason={basePalAddReason || (busy ? m.live_write_in_flight() : undefined)}
				/>
			</Tabs.Panel>
		{/snippet}
	</Tabs>
</div>
