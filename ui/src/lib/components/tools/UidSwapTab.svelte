<script lang="ts">
	import { getAppState, getToastState } from '$states';
	import { Button, Card, Spinner } from '$components/ui';
	import { sendAndWait } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import { HardDrive, Repeat } from 'lucide-svelte';

	const appState = getAppState();
	const toast = getToastState();

	let swapPlayerA: string = $state('');
	let swapPlayerB: string = $state('');
	let isSwapping = $state(false);
	let swapResult: { success?: boolean; error?: string } | null = $state(null);
	let showSwapConfirm = $state(false);

	const hasLoadedSave = $derived(!!appState.saveFile);

	async function handleSwapUids() {
		if (!swapPlayerA || !swapPlayerB || swapPlayerA === swapPlayerB) return;
		showSwapConfirm = false;
		isSwapping = true;
		swapResult = null;
		try {
			const result = await sendAndWait<{ success?: boolean; error?: string }>(
				MessageType.SWAP_PLAYER_UIDS,
				{ old_player_uid: swapPlayerA, new_player_uid: swapPlayerB }
			);
			swapResult = result;
			if (result.error) {
				toast.add(result.error, m.error(), 'error');
			} else if (result.success) {
				toast.add(m.tools_uid_swap_success());
			}
		} catch (err: any) {
			swapResult = { error: err.message };
			toast.add(m.tools_swap_failed({ error: err.message }), m.error(), 'error');
		} finally {
			isSwapping = false;
		}
	}
</script>

<div class="flex flex-col gap-8">
	{#if !hasLoadedSave}
		<Card class="mx-auto max-w-lg">
			<div class="flex flex-col items-center gap-4 p-4">
				<HardDrive size={48} class="text-surface-400" />
				<p class="text-surface-300 text-center">{m.tools_uid_swap_load_first()}</p>
			</div>
		</Card>
	{:else if isSwapping}
		<div class="flex flex-col items-center gap-4">
			<Spinner />
			{#if appState.progressMessage}
				<span class="text-surface-200">{appState.progressMessage}</span>
			{/if}
		</div>
	{:else}
		<section class="w-full">
			<h2 class="text-surface-100 mb-2 text-center text-2xl font-bold">{m.tools_uid_swap_title()}</h2>
			<p class="text-surface-400 mb-6 text-center text-sm">{m.tools_uid_swap_description()}</p>

			<Card class="mx-auto max-w-lg">
				<div class="flex flex-col gap-4 p-4">
					<div class="flex flex-col gap-2">
						<label for="swap-player-a" class="text-surface-300 text-sm font-medium">
							{m.tools_player_a()}
						</label>
						<select
							id="swap-player-a"
							bind:value={swapPlayerA}
							class="bg-surface-800 border-surface-600 text-surface-100 focus:border-primary-500 rounded-lg border px-3 py-2 text-sm focus:outline-none"
						>
							<option value="">{m.tools_select_player()}</option>
							{#each appState.playerSummariesArray as player (player.uid)}
								<option value={player.uid} disabled={player.uid === swapPlayerB}>
									{player.nickname} ({m.tools_player_level({ level: player.level ?? '?' })}) — {player.uid.substring(
										0,
										8
									)}
								</option>
							{/each}
						</select>
					</div>

					<div class="flex items-center justify-center">
						<Repeat size={20} class="text-primary-400" />
					</div>

					<div class="flex flex-col gap-2">
						<label for="swap-player-b" class="text-surface-300 text-sm font-medium">
							{m.tools_player_b()}
						</label>
						<select
							id="swap-player-b"
							bind:value={swapPlayerB}
							class="bg-surface-800 border-surface-600 text-surface-100 focus:border-primary-500 rounded-lg border px-3 py-2 text-sm focus:outline-none"
						>
							<option value="">{m.tools_select_player()}</option>
							{#each appState.playerSummariesArray as player (player.uid)}
								<option value={player.uid} disabled={player.uid === swapPlayerA}>
									{player.nickname} ({m.tools_player_level({ level: player.level ?? '?' })}) — {player.uid.substring(
										0,
										8
									)}
								</option>
							{/each}
						</select>
					</div>

					{#if showSwapConfirm}
						<div class="bg-surface-900 rounded-lg border border-yellow-600/50 p-3">
							<p class="mb-3 text-sm text-yellow-400">{m.tools_uid_swap_warning()}</p>
							<div class="flex justify-end gap-2">
								<Button variant="neutral" size="sm" onclick={() => (showSwapConfirm = false)}>
									{m.cancel()}
								</Button>
								<Button variant="primary" size="sm" onclick={handleSwapUids}>
									{m.tools_confirm_swap()}
								</Button>
							</div>
						</div>
					{:else}
						<Button
							variant="primary"
							onclick={() => (showSwapConfirm = true)}
							disabled={!swapPlayerA || !swapPlayerB || swapPlayerA === swapPlayerB}
						>
							<Repeat size={16} />
							<span>{m.tools_swap_uids()}</span>
						</Button>
					{/if}

					{#if swapResult?.success}
						<div class="border-surface-700 border-t pt-3">
							<p class="text-sm text-green-400">{m.tools_uid_swap_success()}</p>
						</div>
					{/if}

					{#if swapResult?.error}
						<div class="border-surface-700 border-t pt-3">
							<p class="text-sm text-red-400">{swapResult.error}</p>
						</div>
					{/if}
				</div>
			</Card>
		</section>
	{/if}
</div>
