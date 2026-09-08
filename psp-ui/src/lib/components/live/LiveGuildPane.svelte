<script lang="ts">
	import LiveGuildSidebar from './LiveGuildSidebar.svelte';
	import LiveGuildBasePals from './LiveGuildBasePals.svelte';
	import LiveGuildMembers from './LiveGuildMembers.svelte';
	import LiveGuildStorage from './LiveGuildStorage.svelte';
	import LiveGuildChest from './LiveGuildChest.svelte';
	import LiveGuildLab from './LiveGuildLab.svelte';
	import { countResearched, partitionContainers, type LiveGuildView } from './liveGuild.utils';
	import { labResearchData } from '$lib/data';
	import type {
		GameBasePalsJson,
		GameGuildContainersJson,
		GameGuildJson,
		GameGuildsJson,
		GamePalJson,
		GameRefusalJson
	} from '$states/gameState.svelte';
	import * as m from '$i18n/messages';

	let {
		guild = null,
		error = null,
		loading = false,
		basePals = null,
		basePalsError = null,
		basePalsLoading = false,
		basePage = 0,
		onBasePageChange,
		guildContainers = null,
		guildContainersError = null,
		guildContainersLoading = false,
		levelBusy = false,
		setLevelReason,
		onSetBaseCampLevel,
		roleBusy = false,
		setRoleReason,
		onSetMemberRole,
		guilds = null,
		selectedGuildId = null,
		onSelectGuild,
		setItemSlotReason,
		onSetItemSlot,
		busy = false,
		onEditBasePal,
		onAddBasePal,
		basePalEditReason,
		basePalAddReason
	}: {
		guild?: GameGuildJson | null;
		error?: GameRefusalJson | null;
		loading?: boolean;
		basePals?: GameBasePalsJson | null;
		basePalsError?: GameRefusalJson | null;
		basePalsLoading?: boolean;
		basePage?: number;
		onBasePageChange?: (page: number) => void;
		guildContainers?: GameGuildContainersJson | null;
		guildContainersError?: GameRefusalJson | null;
		guildContainersLoading?: boolean;
		levelBusy?: boolean;
		setLevelReason?: string;
		onSetBaseCampLevel?: (level: number) => void;
		roleBusy?: boolean;
		setRoleReason?: string;
		onSetMemberRole?: (memberUid: string, role: string) => void;
		guilds?: GameGuildsJson | null;
		selectedGuildId?: string | null;
		onSelectGuild?: (guildId: string) => void;
		setItemSlotReason?: string;
		onSetItemSlot?: (
			containerId: string,
			slotIndex: number,
			staticItemId: string | null,
			count: number
		) => void;
		busy?: boolean;
		onEditBasePal?: (pal: GamePalJson, slotIndex: number) => void;
		onAddBasePal?: (slotIndex: number) => void;
		basePalEditReason?: string;
		basePalAddReason?: string;
	} = $props();

	let activeView = $state<LiveGuildView>('members');

	const detail = $derived(guild?.guild ?? null);
	const guildChoices = $derived((guilds?.guilds ?? []).filter((entry) => !!entry.id));

	const name = $derived(detail?.name ?? null);
	const baseCount = $derived(detail?.baseCampIds?.length ?? detail?.baseCampPointIds?.length ?? null);
	const members = $derived(detail?.members ?? []);
	const memberUids = $derived(detail?.memberUids ?? []);
	const bases = $derived(detail?.bases ?? []);
	const currentBase = $derived(bases[basePage] ?? null);
	const basePalsMatch = $derived(!!currentBase && basePals?.baseId === currentBase.id);
	const containers = $derived(guildContainers?.containers ?? []);
	const split = $derived(partitionContainers(containers));
	const research = $derived(countResearched(detail?.lab ?? null, labResearchData.research));

	const counts = $derived<Partial<Record<LiveGuildView, string>>>({
		members: String(members.length || memberUids.length),
		lab: `${research.done}/${research.total}`,
		pals: `${basePalsMatch ? (basePals?.pals?.length ?? 0) : 0}/${(basePalsMatch ? basePals?.slotNum : null) ?? currentBase?.palSlotNum ?? 0}`,
		storage: String(split.storage.length),
		chest: String(split.chest.length)
	});
</script>

<div class="h-full rounded-sm">
	<h4 class="h4 mb-2">{m.guild({ count: 1 })}</h4>

	{#if error}
		<p id="live-guild-error" class="text-warning-400 text-sm">{error.error}</p>
	{:else if loading && !detail}
		<p class="text-surface-400 text-sm">{m.loading()}</p>
	{:else if !detail}
		<p class="text-surface-400 text-sm">{m.no_guild_found()}</p>
	{:else}
		<div
			id="live-guild"
			class="@container/guild flex min-h-0 overflow-hidden rounded-sm"
		>
			<LiveGuildSidebar
				{name}
				{guildChoices}
				selectedGuildId={selectedGuildId ?? detail.id}
				{onSelectGuild}
				{baseCount}
				memberCount={members.length || memberUids.length}
				baseCampLevel={detail.baseCampLevel}
				{levelBusy}
				{setLevelReason}
				{onSetBaseCampLevel}
				{counts}
				{activeView}
				onSelectView={(view) => (activeView = view)}
			/>

			<div class="flex min-w-0 flex-1 flex-col">
				{#if activeView === 'members'}
					<div data-testid="live-guild-view-members" class="p-3">
						<LiveGuildMembers
							{members}
							{memberUids}
							adminUid={detail.adminUid}
							roleOptions={detail.roleOptions ??
								[...new Set(members.map((member) => member.role).filter((role): role is string => !!role))]}
							{roleBusy}
							{setRoleReason}
							{onSetMemberRole}
						/>
					</div>
				{:else if activeView === 'lab'}
					<div data-testid="live-guild-view-lab" class="p-3">
						<LiveGuildLab lab={detail.lab} />
					</div>
				{:else if activeView === 'pals'}
					<div data-testid="live-guild-view-pals" class="p-3">
						<LiveGuildBasePals
							{bases}
							{basePage}
							{onBasePageChange}
							{basePals}
							{basePalsError}
							{basePalsLoading}
							{busy}
							onEditPal={onEditBasePal}
							onAddPal={onAddBasePal}
							editDisabledReason={basePalEditReason}
							addDisabledReason={basePalAddReason}
						/>
					</div>
				{:else if activeView === 'storage'}
					<div data-testid="live-guild-view-storage" class="p-3">
						<LiveGuildStorage
							containers={split.storage}
							error={guildContainersError}
							loading={guildContainersLoading}
							{bases}
							{basePage}
							{onBasePageChange}
							onSetSlot={onSetItemSlot}
							editDisabledReason={setItemSlotReason}
						/>
					</div>
				{:else}
					<div data-testid="live-guild-view-chest" class="p-3">
						<LiveGuildChest
							containers={split.chest}
							error={guildContainersError}
							loading={guildContainersLoading}
							onSetSlot={onSetItemSlot}
							editDisabledReason={setItemSlotReason}
						/>
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
