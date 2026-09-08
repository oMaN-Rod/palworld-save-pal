<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import * as m from '$i18n/messages';
	import { Card, Popover, Spinner } from '$components/ui';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { SidebarDetail } from '$components/layout';
	import PalSelectModal from '$components/modals/pal-select/PalSelectModal.svelte';
	import LiveInstanceModal from '$components/live/LiveInstanceModal.svelte';
	import LiveInstanceSwitcher from '$components/live/LiveInstanceSwitcher.svelte';
	import LivePalEditModal from '$components/live/LivePalEditModal.svelte';
	import LivePlayerDetail from '$components/live/LivePlayerDetail.svelte';
	import LivePartyRail from '$components/live/LivePartyRail.svelte';
	import LivePlayerList from '$components/live/LivePlayerList.svelte';
	import LivePlayerSwitcher from '$components/live/LivePlayerSwitcher.svelte';
	import LiveStatusChips from '$components/live/LiveStatusChips.svelte';
	import { cn } from '$theme';
	import { c } from '$lib/utils/commonTranslations';
	import { expData, itemsData, palsData } from '$lib/data';
	import { getGameState, getModalState, getToastState, GameCommandError } from '$states';
	import type {
		GameHealPalsJson,
		GameInstanceFields,
		GameInstanceJson,
		GamePalEditRequest,
		GamePalJson,
		GamePlayerJson
	} from '$states/gameState.svelte';
	import {
		bridgeChipState,
		describeHealResults,
		errorDisplayKey,
		keepsRawMessage,
		opAvailable,
		MAX_LEVEL,
		MIN_LEVEL,
		statusModeLabelKey,
		statusWarningKey,
		type PalAddress,
		type PalAddTarget
	} from '$lib/components/live/liveView.utils';

	const gameState = getGameState();
	const modal = getModalState();
	const toast = getToastState();

	const CHIP_COLOR: Record<string, string> = {
		ok: 'bg-green-500/15 text-green-400',
		read_only: 'bg-yellow-500/15 text-yellow-400',
		offline: 'bg-red-500/15 text-red-400',
		error: 'bg-orange-500/15 text-orange-400'
	};

	const CHIP_LABELS: Record<string, () => string> = {
		ok: m.live_chip_ok,
		read_only: m.live_chip_read_only,
		offline: m.live_chip_offline,
		error: m.live_chip_error
	};

	const MODE_LABELS: Record<string, () => string> = {
		live_mode_solo_host: m.live_mode_solo_host,
		live_mode_dedicated: m.live_mode_dedicated,
		live_mode_client: m.live_mode_client
	};

	const WARNING_LABELS: Record<string, () => string> = {
		live_not_authoritative: m.live_not_authoritative,
		live_world_not_loaded: m.live_world_not_loaded
	};

	const ERROR_LABELS: Record<string, () => string> = {
		live_offline_hint: m.live_offline_hint,
		live_timeout_hint: m.live_timeout_hint,
		live_not_authoritative: m.live_not_authoritative,
		live_capability_unavailable: m.live_capability_unavailable
	};

	function errorText(code: string, message: string): string {
		const key = errorDisplayKey(code);
		if (!key) return message;
		const label = ERROR_LABELS[key]?.() ?? message;
		return keepsRawMessage(code) && message ? `${label} ${message}` : label;
	}

	function commandErrorText(error: unknown): string {
		return error instanceof GameCommandError ? errorText(error.code, error.message) : String(error);
	}

	function reasonText(reason: string | null): string | undefined {
		if (!reason) return undefined;
		return WARNING_LABELS[reason]?.() ?? reason;
	}

	function unavailableReason(op: {
		available: boolean;
		reason: string | null;
	}): string | undefined {
		if (op.available) return undefined;
		return reasonText(op.reason) ?? m.live_capability_unavailable();
	}

	let selectedPlayerUid = $state<string | null>(null);
	let partyExpanded = $state(false);
	let initialLoad = $state(true);
	let detailLoading = $state(false);
	let palsPageLoading = $state(false);
	let detailTab = $state('loadout');
	let basePage = $state(0);
	let basePalsLoading = $state(false);
	let guildContainersLoading = $state(false);
	let guildLevelBusy = $state(false);
	let guildRoleBusy = $state(false);
	let selectedGuildId = $state<string | null>(null);
	let healAllBusy = $state(false);
	let healingPalId = $state<string | null>(null);
	let movePendingSlot = $state<number | null>(null);
	let levelBusy = $state(false);
	let instanceSelectPending = $state(false);
	let editingInstanceId = $state<string | null>(null);
	let instancesPolling = false;

	const chipState = $derived(bridgeChipState(gameState.status, gameState.statusError));
	const modeLabelKey = $derived(
		gameState.status ? statusModeLabelKey(gameState.status.mode) : null
	);
	const warningKey = $derived(statusWarningKey(gameState.status));

	const healAvailable = $derived(opAvailable(gameState.status, gameState.capabilities, 'pal.heal'));
	const setLevelAvailable = $derived(
		opAvailable(gameState.status, gameState.capabilities, 'player.edit')
	);
	const setGuildRoleAvailable = $derived(
		opAvailable(gameState.status, gameState.capabilities, 'guild.setRole')
	);
	const editGuildAvailable = $derived(
		opAvailable(gameState.status, gameState.capabilities, 'guild.edit')
	);
	const removePalAvailable = $derived(
		opAvailable(gameState.status, gameState.capabilities, 'pal.remove')
	);
	const movePalAvailable = $derived(
		opAvailable(gameState.status, gameState.capabilities, 'pal.move')
	);
	const editPalAvailable = $derived(
		opAvailable(gameState.status, gameState.capabilities, 'pal.edit')
	);
	const addPalAvailable = $derived(
		opAvailable(gameState.status, gameState.capabilities, 'pal.add')
	);
	const setItemSlotAvailable = $derived(
		opAvailable(gameState.status, gameState.capabilities, 'item.setSlot')
	);

	const worldPendingNote = $derived(
		gameState.status && !gameState.status.worldLoaded ? m.live_world_not_loaded() : undefined
	);

	const selectedPlayer = $derived(
		gameState.players.find((player) => player.uid === selectedPlayerUid) ?? null
	);

	const sidebarWidth = $derived(
		!selectedPlayer ? 'sm:w-72 md:w-100' : partyExpanded ? 'sm:w-90' : 'sm:w-20'
	);

	const activeInstanceConnected = $derived(gameState.status !== null);
	const activeInstance = $derived(
		gameState.instances.find((instance) => instance.id === gameState.activeInstanceId) ?? null
	);

	async function refresh() {
		await Promise.all([
			gameState.refreshStatus(),
			gameState.refreshCapabilities(),
			gameState.refreshPlayers()
		]);
		initialLoad = false;
		await refreshSelectedDetail();
	}

	async function refreshSelectedDetail() {
		const uid = selectedPlayerUid;
		if (!uid || detailLoading || palsPageLoading || gameState.writeBusy) return;
		if (detailTab === 'palbox') {
			await gameState.loadPals(uid, gameState.palsPage);
		} else if (detailTab === 'guild') {
			await loadGuildDetail();
			await Promise.all([loadBaseForPage(basePage, true), loadGuildContainers(true)]);
		} else {
			await gameState.loadInventory(uid);
		}
	}

	async function pollInstances() {
		if (instancesPolling) return;
		instancesPolling = true;
		try {
			await gameState.refreshInstances();
		} finally {
			instancesPolling = false;
		}
	}

	function defaultInstanceFields(): GameInstanceFields {
		return { name: '', host: '127.0.0.1', port: 8788, token: '' };
	}

	async function saveInstance(fields: GameInstanceFields) {
		try {
			if (editingInstanceId) {
				await gameState.updateInstance(editingInstanceId, fields);
			} else {
				await gameState.addInstance(fields);
			}
			modal.closeModal();
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		}
	}

	function openAddInstanceModal() {
		editingInstanceId = null;
		// @ts-ignore
		void modal.showModal(LiveInstanceModal, {
			initial: defaultInstanceFields(),
			ontest: (fields: GameInstanceFields) => gameState.testInstance(fields),
			onsave: saveInstance,
			oncancel: () => modal.closeModal()
		});
	}

	function openEditInstanceModal(instance: GameInstanceJson) {
		editingInstanceId = instance.id;
		// @ts-ignore
		void modal.showModal(LiveInstanceModal, {
			initial: { name: instance.name, host: instance.host, port: instance.port, token: '' },
			editing: true,
			ontest: (fields: GameInstanceFields) => gameState.testInstance(fields),
			onsave: saveInstance,
			oncancel: () => modal.closeModal()
		});
	}

	async function selectInstance(id: string) {
		if (instanceSelectPending) return;
		instanceSelectPending = true;
		try {
			await gameState.selectInstance(id);
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		} finally {
			instanceSelectPending = false;
		}
	}

	onMount(() => {
		void refresh();
		void pollInstances();
	});

	$effect(() => {
		const interval = setInterval(() => void refresh(), 5000);
		return () => clearInterval(interval);
	});

	$effect(() => {
		const interval = setInterval(() => void pollInstances(), 5000);
		return () => clearInterval(interval);
	});

	async function selectPlayer(player: GamePlayerJson) {
		selectedPlayerUid = player.uid;
		detailLoading = true;
		try {
			basePage = 0;
			selectedGuildId = null;
			await Promise.all([
				gameState.loadPals(player.uid, 0),
				gameState.loadInventory(player.uid),
				gameState.loadGuild({ playerUid: player.uid }),
				gameState.loadGuilds()
			]);
			await Promise.all([loadBaseForPage(0), loadGuildContainers()]);
		} finally {
			detailLoading = false;
		}
	}

	async function loadBaseForPage(page: number, background = false): Promise<void> {
		const baseId = gameState.guild?.guild?.bases?.[page]?.id;
		if (!baseId) return;
		if (background) {
			await gameState.loadBasePals(baseId);
			return;
		}
		basePalsLoading = true;
		try {
			await gameState.loadBasePals(baseId);
		} finally {
			basePalsLoading = false;
		}
	}

	async function loadGuildDetail(): Promise<void> {
		if (selectedGuildId) {
			await gameState.loadGuild({ guildId: selectedGuildId });
			return;
		}
		const uid = selectedPlayerUid;
		if (uid) await gameState.loadGuild({ playerUid: uid });
	}

	async function selectGuild(guildId: string): Promise<void> {
		selectedGuildId = guildId;
		basePage = 0;
		await loadGuildDetail();
		await Promise.all([loadBaseForPage(0), loadGuildContainers()]);
	}

	async function loadGuildContainers(background = false): Promise<void> {
		const guildId = gameState.guild?.guild?.id;
		if (!guildId) return;
		if (background) {
			await gameState.loadGuildContainers(guildId);
			return;
		}
		guildContainersLoading = true;
		try {
			await gameState.loadGuildContainers(guildId);
		} finally {
			guildContainersLoading = false;
		}
	}

	async function setBaseCampLevel(level: number): Promise<void> {
		const guildId = gameState.guild?.guild?.id;
		if (!guildId || !editGuildAvailable.available || gameState.writeBusy) return;
		guildLevelBusy = true;
		try {
			const result = await gameState.editGuild(guildId, level);
			if (result.verified) toast.add(m.live_level_set({ level }), m.success(), 'success');
			else toast.add(m.live_change_unconfirmed(), undefined, 'warning');
			void loadGuildDetail();
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		} finally {
			guildLevelBusy = false;
		}
	}

	async function setMemberRole(memberUid: string, role: string): Promise<void> {
		const guildId = gameState.guild?.guild?.id;
		if (!guildId || !setGuildRoleAvailable.available || gameState.writeBusy) return;
		guildRoleBusy = true;
		try {
			const result = await gameState.setGuildRole(guildId, memberUid, role);
			if (result.verified) toast.add(m.live_guild_role_set({ role }), m.success(), 'success');
			else toast.add(m.live_change_unconfirmed(), undefined, 'warning');
			void loadGuildDetail();
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		} finally {
			guildRoleBusy = false;
		}
	}

	async function changeBasePage(page: number): Promise<void> {
		basePage = page;
		await loadBaseForPage(page);
	}

	async function changePalsPage(page: number) {
		const uid = selectedPlayerUid;
		if (!uid) return;
		palsPageLoading = true;
		try {
			await gameState.loadPals(uid, page);
		} finally {
			palsPageLoading = false;
		}
	}

	async function setLevel(level: number): Promise<boolean> {
		const uid = selectedPlayerUid;
		if (!uid || !setLevelAvailable.available || gameState.writeBusy) return false;
		if (!Number.isInteger(level) || level < MIN_LEVEL || level > MAX_LEVEL) {
			toast.add(m.live_level_out_of_range({ min: MIN_LEVEL, max: MAX_LEVEL }), m.error(), 'error');
			return false;
		}
		const totalExp = (await expData.getExpDataByLevel(level))?.TotalEXP;
		if (totalExp === undefined) {
			toast.add(m.live_level_out_of_range({ min: MIN_LEVEL, max: MAX_LEVEL }), m.error(), 'error');
			return false;
		}
		levelBusy = true;
		try {
			const result = await gameState.editPlayer(uid, level, totalExp);
			if (result.verified) {
				toast.add(m.live_level_set({ level }), m.success(), 'success');
			} else {
				toast.add(m.live_change_unconfirmed(), undefined, 'warning');
			}
			void gameState.refreshPlayers();
			return true;
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
			return false;
		} finally {
			levelBusy = false;
		}
	}

	function reportHealResult(response: GameHealPalsJson) {
		const { healed, unconfirmed, failed } = describeHealResults(response.results);
		const counted = m.live_healed_count({ healed, total: response.results.length });
		const message =
			unconfirmed > 0
				? `${counted} — ${m.live_healed_unconfirmed({ count: unconfirmed })}`
				: counted;
		if (failed > 0) toast.add(message, m.error(), 'error');
		else if (unconfirmed > 0) toast.add(message, undefined, 'warning');
		else toast.add(message, m.success(), 'success');
	}

	async function heal(targets: { slot_index: number }[]) {
		const uid = selectedPlayerUid;
		if (!uid || !healAvailable.available || gameState.writeBusy || targets.length === 0) return;
		try {
			reportHealResult(await gameState.healPals(uid, targets));
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		}
	}

	async function healOne(pal: GamePalJson) {
		healingPalId = pal.instanceId;
		try {
			await heal([{ slot_index: pal.slotIndex }]);
		} finally {
			healingPalId = null;
		}
	}

	async function healAll() {
		healAllBusy = true;
		try {
			await heal(gameState.pals.map((pal) => ({ slot_index: pal.slotIndex })));
		} finally {
			healAllBusy = false;
		}
	}

	async function reloadPals() {
		const uid = selectedPlayerUid;
		if (uid) await gameState.loadPals(uid, gameState.palsPage);
	}

	async function removePal(pal: GamePalJson) {
		const uid = selectedPlayerUid;
		if (!uid || !removePalAvailable.available || gameState.writeBusy) return;
		const name = pal.nickname || pal.characterId;
		const confirmed = await modal.showConfirmModal({
			title: m.live_remove_pal(),
			message: m.delete_entity_by_name_confirm({ name }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (!confirmed) return;
		try {
			const result = await gameState.removePal(uid, pal.slotIndex);
			if (result.verified) {
				toast.add(m.live_pal_removed({ name }), m.success(), 'success');
			} else {
				toast.add(m.live_change_unconfirmed(), undefined, 'warning');
			}
			await reloadPals();
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		}
	}

	async function movePal(slotIndex: number) {
		const uid = selectedPlayerUid;
		if (!uid || !movePalAvailable.available) return;
		if (movePendingSlot === null) {
			movePendingSlot = slotIndex;
			return;
		}
		if (movePendingSlot === slotIndex) {
			movePendingSlot = null;
			return;
		}
		const from = movePendingSlot;
		movePendingSlot = null;
		if (gameState.writeBusy) return;
		try {
			const result = await gameState.movePal(uid, from, slotIndex);
			if (!result.verified) {
				toast.add(m.live_change_unconfirmed(), undefined, 'warning');
			}
			await reloadPals();
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		}
	}

	async function editPal(
		pal: GamePalJson,
		address: PalAddress,
		options: { playerUid?: string | null; reload?: () => Promise<void> } = {}
	) {
		const uid = options.playerUid || selectedPlayerUid;
		if (!uid || !editPalAvailable.available || gameState.writeBusy) return;
		const detail = await gameState.loadPalDetail(uid, address);
		if (!detail) {
			toast.add(m.live_pal_detail_unavailable(), m.error(), 'error');
			return;
		}
		// @ts-ignore
		const changes = await modal.showModal<GamePalEditRequest | undefined>(LivePalEditModal, {
			pal,
			detail,
			levelCap: selectedPlayer?.level ?? 0
		});
		if (!changes || Object.keys(changes).length === 0) return;
		try {
			const result = await gameState.editPal(uid, address, changes);
			if (result.verified) {
				toast.add(m.live_pal_edited(), m.success(), 'success');
			} else {
				toast.add(m.live_change_unconfirmed(), undefined, 'warning');
			}
			await reloadPals();
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		}
	}

	const editBoxPal = (pal: GamePalJson, slotIndex: number) => editPal(pal, { slotIndex });
	const editPartyPal = (pal: GamePalJson) => editPal(pal, { instanceId: pal.instanceId });
	const editBasePal = (pal: GamePalJson) =>
		editPal(
			pal,
			{ instanceId: pal.instanceId },
			{ playerUid: pal.playerUid, reload: () => loadBaseForPage(basePage) }
		);

	async function addPalTo(
		slotIndex: number | null,
		target: PalAddTarget,
		reload: () => Promise<void>
	) {
		const uid = selectedPlayerUid;
		if (!uid || !addPalAvailable.available || gameState.writeBusy) return;
		// @ts-ignore
		const result = await modal.showModal<[string, string, string] | undefined>(PalSelectModal, {
			title: m.live_add_pal(),
			showNickname: false,
			showGender: true
		});
		if (!result) return;
		const [characterId, , gender] = result;
		if (!characterId) return;
		try {
			const added = await gameState.addPal(
				uid,
				slotIndex,
				characterId,
				undefined,
				gender?.toLowerCase(),
				target
			);
			const name = palsData.getByKey(characterId)?.localized_name ?? characterId;
			if (added.verified) {
				toast.add(m.live_pal_added({ name }), m.success(), 'success');
			} else {
				toast.add(m.live_change_unconfirmed(), undefined, 'warning');
			}
			await reload();
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		}
	}

	const addPal = (slotIndex: number) => addPalTo(slotIndex, {}, reloadPals);

	const addPartyPal = () => addPalTo(null, { party: true }, reloadPals);

	async function addBasePal(slotIndex: number) {
		const baseId = gameState.guild?.guild?.bases?.[basePage]?.id;
		if (!baseId) return;
		await addPalTo(slotIndex, { baseId }, () => loadBaseForPage(basePage));
	}

	async function setItemSlot(
		containerId: string,
		slotIndex: number,
		staticItemId: string | null,
		count: number
	) {
		const uid = selectedPlayerUid;
		if (!uid || !setItemSlotAvailable.available || gameState.writeBusy) return;
		try {
			const result = await gameState.setItemSlot(uid, containerId, slotIndex, staticItemId, count);
			if (result.verified) {
				if (staticItemId) {
					const name = itemsData.getByKey(staticItemId)?.info.localized_name ?? staticItemId;
					toast.add(m.live_slot_set({ count, item: name }), m.success(), 'success');
				} else {
					toast.add(m.live_slot_cleared(), m.success(), 'success');
				}
			} else {
				toast.add(m.live_change_unconfirmed(), undefined, 'warning');
			}
			if (detailTab === 'guild') void loadGuildContainers(true);
			else void gameState.loadInventory(uid);
		} catch (error) {
			toast.add(commandErrorText(error), m.error(), 'error');
		}
	}
</script>

<div class="animate-fade-in flex h-full flex-col">
	<div class="flex items-center gap-3 px-4 pt-4">
		<div class="flex shrink-0 items-center gap-2">
			<Icon icon="tabler:activity" size={20} class="text-primary-400" />
			<div>
				<h1 class="heading-gradient text-xl font-bold">{m.live_title()}</h1>
				<p class="text-surface-400 text-sm">
					{m.live_players_online({ count: gameState.players.length })}
				</p>
			</div>
		</div>

		<Popover position="bottom-start" popoverClass="w-80 max-h-96 overflow-y-auto">
			<button
				type="button"
				id="live-instance-switcher-trigger"
				aria-label={m.live_instance_switch()}
				class="bg-surface-800 hover:bg-surface-700 flex shrink-0 items-center gap-2 rounded-sm px-2 py-1 text-xs"
			>
				<Icon icon="tabler:server-2" size={16} />
				<span class="max-w-32 truncate">{activeInstance?.name ?? m.live_instance_switch()}</span>
				<Icon icon="tabler:chevron-down" size={14} class="text-surface-400" />
			</button>

			{#snippet content({ close }: { close: () => void })}
				<LiveInstanceSwitcher
					instances={gameState.instances}
					activeId={gameState.activeInstanceId}
					activeConnected={activeInstanceConnected}
					pending={instanceSelectPending}
					onselect={(id) => {
						close();
						void selectInstance(id);
					}}
					onedit={(instance) => {
						close();
						openEditInstanceModal(instance);
					}}
					onadd={() => {
						close();
						openAddInstanceModal();
					}}
				/>
			{/snippet}
		</Popover>

		{#if selectedPlayer}
			<div class="flex min-w-0 items-center gap-3" in:fade={{ delay: 150, duration: 250 }}>
				<LivePlayerSwitcher
					players={gameState.players}
					selected={selectedPlayer}
					onSelect={selectPlayer}
				/>
				{#if gameState.status}
					<LiveStatusChips status={gameState.status} />
				{/if}
			</div>
		{/if}

		<span
			class={cn(
				'ml-auto shrink-0 rounded-sm px-2 py-0.5 text-xs font-medium',
				CHIP_COLOR[chipState]
			)}
		>
			{CHIP_LABELS[chipState]?.()}
		</span>
	</div>

	<div class="min-h-0 flex-1">
		<SidebarDetail
			detailActive={selectedPlayer !== null}
			sidebarClass={sidebarWidth}
		>
			{#snippet sidebar()}
				{#if selectedPlayer}
					<div in:fade={{ delay: 150, duration: 250 }}>
						<LivePartyRail
							party={gameState.party}
							partyReadable={gameState.partyReadable}
							levelCap={selectedPlayer.level ?? 0}
							healBusy={gameState.writeBusy}
							{healingPalId}
							healDisabledReason={unavailableReason(healAvailable)}
							editDisabledReason={gameState.writeBusy
								? m.live_write_in_flight()
								: unavailableReason(editPalAvailable)}
							addDisabledReason={gameState.writeBusy
								? m.live_write_in_flight()
								: unavailableReason(addPalAvailable)}
							bind:expanded={partyExpanded}
							onHeal={healOne}
							onEdit={editPalAvailable.available ? editPartyPal : undefined}
							onAdd={addPalAvailable.available ? addPartyPal : undefined}
						/>
					</div>
				{:else}
					<div class="flex flex-col gap-4">
						<Card>
							<div class="flex gap-2">
								<h2 class="mb-2 text-lg font-semibold">{m.live_status_title()}</h2>
								<p class="text-surface-500 mt-2 text-xs">v{gameState.status?.modVersion}</p>
							</div>
							{#if gameState.status}
								<dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
									<dt class="text-surface-400">{m.live_status_mode()}</dt>
									<dd class="truncate text-right">
										{modeLabelKey ? MODE_LABELS[modeLabelKey]?.() : gameState.status.mode}
									</dd>
									<dt class="text-surface-400">{m.live_status_authority()}</dt>
									<dd
										class={cn(
											'truncate text-right',
											gameState.status.authoritative ? '' : 'text-warning-400'
										)}
									>
										{gameState.status.authoritative
											? m.live_status_host()
											: m.live_status_observer()}
									</dd>
									<dt class="text-surface-400">{m.live_status_world()}</dt>
									<dd
										class={cn(
											'truncate text-right',
											gameState.status.worldLoaded ? '' : 'text-warning-400'
										)}
									>
										{gameState.status.worldLoaded
											? m.live_status_world_loaded()
											: m.live_status_world_pending()}
									</dd>
									<dt class="text-surface-400">{m.live_status_queue()}</dt>
									<dd class="text-right">{gameState.status.queueDepth}</dd>
								</dl>

								{#if warningKey}
									<p class="text-warning-400 mt-2 text-sm">{WARNING_LABELS[warningKey]?.()}</p>
								{/if}
							{:else if gameState.statusError}
								<p class="text-error-400 text-sm">
									{errorText(gameState.statusError.code, gameState.statusError.error)}
								</p>
							{:else}
								<div class="flex items-center gap-2 py-2">
									<Spinner size="size-6" />
									<span class="text-surface-300 text-sm">{m.loading()}</span>
								</div>
							{/if}
						</Card>

						<Card>
							<h2 class="mb-3 text-lg font-semibold">{m.live_players_title()}</h2>
							{#if gameState.playersError}
								<p class="text-error-400 text-sm">
									{errorText(gameState.playersError.code, gameState.playersError.error)}
								</p>
							{:else if initialLoad}
								<div class="flex flex-col items-center gap-3 py-6">
									<Spinner size="size-16" />
									<p class="text-surface-300 text-sm">
										{m.loading_entity({ entity: c.players })}
									</p>
								</div>
							{:else if gameState.players.length === 0}
								<p class="text-surface-400 text-sm">{m.no_entity_yet({ entity: c.players })}</p>
							{:else}
								{#if worldPendingNote}
									<p class="text-warning-400 mb-2 text-xs">{worldPendingNote}</p>
								{/if}
								<LivePlayerList
									players={gameState.players}
									selectedUid={selectedPlayerUid}
									onSelect={selectPlayer}
								/>
							{/if}
						</Card>
					</div>
				{/if}
			{/snippet}

			{#snippet detail()}
				{#if selectedPlayer}
					{#key selectedPlayer.uid}
						<LivePlayerDetail
							player={selectedPlayer}
							pals={gameState.pals}
							palsPage={gameState.palsPage}
							palsPageCount={gameState.palsPageCount}
							palsSlotCount={gameState.palsSlotCount}
							palsSlotBase={gameState.palsSlotBase}
							palsError={gameState.palsError
								? errorText(gameState.palsError.code, gameState.palsError.error)
								: undefined}
							inventory={gameState.inventory}
							inventoryError={gameState.inventoryError
								? errorText(gameState.inventoryError.code, gameState.inventoryError.error)
								: undefined}
							loading={detailLoading}
							palsLoading={palsPageLoading}
							guild={gameState.guild}
							guildError={gameState.guildError}
							basePals={gameState.basePals}
							basePalsError={gameState.basePalsError}
							{basePalsLoading}
							{basePage}
							onBasePageChange={changeBasePage}
							guildContainers={gameState.guildContainers}
							guildContainersError={gameState.guildContainersError}
							{guildContainersLoading}
							{guildLevelBusy}
							setGuildLevelReason={unavailableReason(editGuildAvailable)}
							onSetBaseCampLevel={setBaseCampLevel}
							{guildRoleBusy}
							setGuildRoleReason={unavailableReason(setGuildRoleAvailable)}
							onSetMemberRole={setMemberRole}
							guilds={gameState.guilds}
							{selectedGuildId}
							onSelectGuild={selectGuild}
							bind:tab={detailTab}
							busy={gameState.writeBusy}
							healReason={unavailableReason(healAvailable)}
							setLevelReason={unavailableReason(setLevelAvailable)}
							setItemSlotReason={unavailableReason(setItemSlotAvailable)}
							{healAllBusy}
							{healingPalId}
							{levelBusy}
							onHeal={healOne}
							onHealAll={healAll}
							onPalsPageChange={changePalsPage}
							onSetLevel={setLevel}
							onSetItemSlot={setItemSlot}
							onRemovePal={removePal}
							onMovePal={movePal}
							onAddPal={addPalAvailable.available ? addPal : undefined}
							onEditPal={editPalAvailable.available ? editBoxPal : undefined}
							onAddBasePal={addPalAvailable.available ? addBasePal : undefined}
							onEditBasePal={editPalAvailable.available ? editBasePal : undefined}
							basePalAddReason={unavailableReason(addPalAvailable)}
							basePalEditReason={unavailableReason(editPalAvailable)}
							palEditReason={unavailableReason(editPalAvailable)}
							{movePendingSlot}
							removePalReason={unavailableReason(removePalAvailable)}
							movePalReason={unavailableReason(movePalAvailable)}
							addPalReason={unavailableReason(addPalAvailable)}
						/>
					{/key}
				{/if}
			{/snippet}
		</SidebarDetail>
	</div>
</div>
