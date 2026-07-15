<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getAppState, getToastState } from '$states';
	import { Button, Card, Spinner } from '$components/ui';
	import { sendAndWait } from '$lib/utils/websocketUtils';
	import { MessageType, type PlayerSummary } from '$types';
	import { c } from '$lib/utils/commonTranslations';
	import * as m from '$i18n/messages';
	import { Monitor, Upload } from 'lucide-svelte';

	const appState = getAppState();
	const toast = getToastState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	const hasLoadedSave = $derived(!!appState.saveFile);

	let transferStep: 'select' | 'players' | 'done' = $state('select');
	let isLoadingTransfer = $state(false);
	let sourceWorldName = $state('');
	let targetWorldName = $state('');
	let sourcePlayers: Record<string, PlayerSummary> = $state({});
	let standaloneTargetPlayers: Record<string, PlayerSummary> = $state({});
	let sourceLoaded = $state(false);
	let standaloneTargetLoaded = $state(false);
	let selectedSourcePlayer = $state('');
	let selectedTargetPlayer = $state('');
	let transferOpts = $state({
		character: true,
		inventory: true,
		pals: true,
		tech: true,
		appearance: true
	});
	let isTransferring = $state(false);
	let transferResult: { success?: boolean; error?: string } | null = $state(null);

	const sourcePlayersArray = $derived(Object.values(sourcePlayers));
	const standaloneTargetPlayersArray = $derived(Object.values(standaloneTargetPlayers));
	const useStandaloneTarget = $derived(!hasLoadedSave);
	const targetPlayersArray = $derived(
		useStandaloneTarget ? standaloneTargetPlayersArray : appState.playerSummariesArray
	);
	const readyForPlayerSelect = $derived(
		sourceLoaded && (!useStandaloneTarget || standaloneTargetLoaded)
	);

	const transferOptionDefs: { key: keyof typeof transferOpts; label: string }[] = [
		{ key: 'character', label: m.character() },
		{ key: 'inventory', label: m.inventory() },
		{ key: 'pals', label: c.pals },
		{ key: 'tech', label: m.technology({ count: 1 }) },
		{ key: 'appearance', label: m.appearance() }
	];

	async function handleLoadTransferSave(role: 'source' | 'target') {
		isLoadingTransfer = true;
		transferResult = null;
		try {
			const result = await sendAndWait<{
				success?: boolean;
				error?: string;
				canceled?: boolean;
				role?: string;
				player_count?: number;
				world_name?: string;
			}>(MessageType.LOAD_SOURCE_SAVE, { type: 'steam', path: '__select__', role });

			if (result.canceled) {
				return;
			}
			if (result.error) {
				toast.add(result.error, m.error(), 'error');
			} else if (result.success) {
				const playersResult = await sendAndWait<{
					source?: Record<string, PlayerSummary>;
					target?: Record<string, PlayerSummary>;
				}>(MessageType.GET_SOURCE_PLAYERS, {});

				if (role === 'source') {
					sourceWorldName = result.world_name ?? m.unknown();
					sourcePlayers = playersResult?.source ?? {};
					sourceLoaded = true;
				} else {
					targetWorldName = result.world_name ?? m.unknown();
					standaloneTargetPlayers = playersResult?.target ?? {};
					standaloneTargetLoaded = true;
				}

				if (readyForPlayerSelect) {
					transferStep = 'players';
				}
			}
		} catch (err: any) {
			toast.add(m.tools_load_failed({ role, error: err.message }), m.error(), 'error');
		} finally {
			isLoadingTransfer = false;
		}
	}

	async function handleTransfer() {
		if (!selectedSourcePlayer) return;
		isTransferring = true;
		transferResult = null;
		try {
			const result = await sendAndWait<{ success?: boolean; error?: string }>(
				MessageType.TRANSFER_PLAYER,
				{
					source_player_uid: selectedSourcePlayer,
					target_player_uid: selectedTargetPlayer || null,
					transfer_character: transferOpts.character,
					transfer_inventory: transferOpts.inventory,
					transfer_pals: transferOpts.pals,
					transfer_tech: transferOpts.tech,
					transfer_appearance: transferOpts.appearance
				}
			);
			transferResult = result;
			if (result.error) {
				toast.add(result.error, m.error(), 'error');
			} else if (result.success) {
				toast.add(m.tools_transfer_success());
				transferStep = 'done';
			}
		} catch (err: any) {
			transferResult = { error: err.message };
			toast.add(m.tools_transfer_failed({ error: err.message }), m.error(), 'error');
		} finally {
			isTransferring = false;
		}
	}

	function resetTransfer() {
		transferStep = 'select';
		sourcePlayers = {};
		standaloneTargetPlayers = {};
		sourceLoaded = false;
		standaloneTargetLoaded = false;
		sourceWorldName = '';
		targetWorldName = '';
		selectedSourcePlayer = '';
		selectedTargetPlayer = '';
		transferResult = null;
		sendAndWait(MessageType.UNLOAD_SOURCE_SAVE, {});
	}
</script>

<div class="flex flex-col gap-8">
	{#if !isDesktopMode}
		<Card class="mx-auto max-w-lg">
			<div class="flex flex-col items-center gap-4 p-4">
				<Monitor size={48} class="text-surface-400" />
				<p class="text-surface-300 text-center">{m.tools_transfer_desktop_required()}</p>
			</div>
		</Card>
	{:else if isLoadingTransfer || isTransferring}
		<div class="flex flex-col items-center gap-4">
			<Spinner />
			{#if appState.progressMessage}
				<span class="text-surface-200">{appState.progressMessage}</span>
			{/if}
		</div>
	{:else}
		<section class="w-full">
			<p class="text-surface-400 mb-6 text-center text-sm">
				{m.tools_transfer_description()}
				{#if hasLoadedSave}
					{m.tools_transfer_target_loaded_hint()}
				{/if}
			</p>

			{#if transferStep === 'select'}
				<div class="mx-auto flex max-w-2xl flex-col gap-6">
					<div class="grid grid-cols-1 gap-6 sm:grid-cols-2">
						<!-- Source save -->
						<Card>
							<div class="flex flex-col items-center gap-4 p-4">
								<h3 class="text-surface-200 font-semibold">{m.tools_source_save()}</h3>
								{#if sourceLoaded}
									<p class="text-sm text-green-400">
										{sourceWorldName} ({m.tools_players_count({ count: sourcePlayersArray.length })})
									</p>
									<button
										class="text-surface-400 hover:text-surface-200 text-xs"
										onclick={() => {
											sourceLoaded = false;
											sourcePlayers = {};
											sourceWorldName = '';
										}}
									>
										{m.tools_change()}
									</button>
								{:else}
									<p class="text-surface-400 text-center text-sm">{m.tools_select_source_hint()}</p>
									<Button variant="primary" onclick={() => handleLoadTransferSave('source')}>
										<Upload size={16} />
										<span>{m.tools_select_source()}</span>
									</Button>
								{/if}
							</div>
						</Card>

						<!-- Target save -->
						<Card>
							<div class="flex flex-col items-center gap-4 p-4">
								<h3 class="text-surface-200 font-semibold">{m.tools_target_save()}</h3>
								{#if hasLoadedSave}
									<p class="text-sm text-green-400">
										{appState.saveFile?.world_name ?? m.tools_loaded_save()} ({m.tools_players_count({
											count: appState.playerSummariesArray.length
										})})
									</p>
									<span class="text-surface-500 text-xs">{m.tools_using_loaded_save()}</span>
								{:else if standaloneTargetLoaded}
									<p class="text-sm text-green-400">
										{targetWorldName} ({m.tools_players_count({
											count: standaloneTargetPlayersArray.length
										})})
									</p>
									<button
										class="text-surface-400 hover:text-surface-200 text-xs"
										onclick={() => {
											standaloneTargetLoaded = false;
											standaloneTargetPlayers = {};
											targetWorldName = '';
										}}
									>
										{m.tools_change()}
									</button>
								{:else}
									<p class="text-surface-400 text-center text-sm">{m.tools_select_target_hint()}</p>
									<Button variant="primary" onclick={() => handleLoadTransferSave('target')}>
										<Upload size={16} />
										<span>{m.tools_select_target()}</span>
									</Button>
								{/if}
							</div>
						</Card>
					</div>

					{#if readyForPlayerSelect}
						<Button variant="primary" class="mx-auto" onclick={() => (transferStep = 'players')}>
							{m.tools_continue_player_selection()}
						</Button>
					{/if}
				</div>
			{:else if transferStep === 'players'}
				<div class="mx-auto flex max-w-2xl flex-col gap-6">
					<div class="flex items-center justify-between">
						<span class="text-surface-400 text-sm">
							{sourceWorldName} → {hasLoadedSave
								? (appState.saveFile?.world_name ?? m.tools_loaded_save())
								: targetWorldName}
						</span>
						<button
							class="text-surface-400 hover:text-surface-200 text-sm"
							onclick={resetTransfer}
						>
							{m.tools_start_over()}
						</button>
					</div>

					<div class="grid grid-cols-1 gap-6 sm:grid-cols-2">
						<!-- Source player -->
						<Card>
							<div class="flex flex-col gap-3 p-4">
								<h3 class="text-surface-200 text-sm font-semibold">{m.tools_source_player()}</h3>
								<select
									bind:value={selectedSourcePlayer}
									class="bg-surface-800 border-surface-600 text-surface-100 focus:border-primary-500 rounded-lg border px-3 py-2 text-sm focus:outline-none"
								>
									<option value="">{m.tools_select_player()}</option>
									{#each sourcePlayersArray as player (player.uid)}
										<option value={player.uid}>
											{player.nickname} ({m.tools_player_level({ level: player.level ?? '?' })})
										</option>
									{/each}
								</select>
							</div>
						</Card>

						<!-- Target player -->
						<Card>
							<div class="flex flex-col gap-3 p-4">
								<h3 class="text-surface-200 text-sm font-semibold">{m.tools_target_player()}</h3>
								<select
									bind:value={selectedTargetPlayer}
									class="bg-surface-800 border-surface-600 text-surface-100 focus:border-primary-500 rounded-lg border px-3 py-2 text-sm focus:outline-none"
								>
									<option value="">{m.tools_new_player_spawn()}</option>
									{#each targetPlayersArray as player (player.uid)}
										<option value={player.uid}>
											{player.nickname} ({m.tools_player_level({ level: player.level ?? '?' })})
										</option>
									{/each}
								</select>
								<span class="text-surface-500 text-xs">{m.tools_new_player_hint()}</span>
							</div>
						</Card>
					</div>

					<!-- Transfer options -->
					<Card>
						<div class="flex flex-col gap-3 p-4">
							<h3 class="text-surface-200 text-sm font-semibold">{m.tools_transfer_options()}</h3>
							<div class="grid grid-cols-2 gap-2 sm:grid-cols-3">
								{#each transferOptionDefs as opt (opt.key)}
									<label class="text-surface-300 flex items-center gap-2 text-sm">
										<input
											type="checkbox"
											bind:checked={transferOpts[opt.key]}
											class="accent-primary-500"
										/>
										{opt.label}
									</label>
								{/each}
							</div>
						</div>
					</Card>

					<Button
						variant="primary"
						class="mx-auto"
						onclick={handleTransfer}
						disabled={!selectedSourcePlayer}
					>
						<Upload size={16} />
						<span>{m.tools_transfer_player()}</span>
					</Button>
				</div>
			{:else if transferStep === 'done'}
				<Card class="mx-auto max-w-lg">
					<div class="flex flex-col items-center gap-4 p-4">
						{#if transferResult?.success}
							<p class="text-center text-green-400">{m.tools_transfer_success()}</p>
						{/if}
						{#if transferResult?.error}
							<p class="text-center text-red-400">{transferResult.error}</p>
						{/if}
						<Button variant="neutral" onclick={resetTransfer}>{m.tools_transfer_another()}</Button>
					</div>
				</Card>
			{/if}
		</section>
	{/if}
</div>
