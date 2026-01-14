<script lang="ts">
	import { Card, Combobox } from '$components/ui';
	import { Save, X, Upload, Share, Download } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { getAppState } from '$states';
	import type { UPSPal, Player } from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		title = m.export_pals({ count: "", pals: c.pals }),
		message = '',
		pals = [],
		closeModal
	}: {
		title?: string;
		message?: string;
		pals: UPSPal[];
		closeModal: (value: any) => void;
	} = $props();

	const appState = getAppState();

	let modalContainer: HTMLDivElement;
	let exportTarget: 'pal_box' | 'dps' | 'gps' = $state('pal_box');
	let selectedPlayerId: string | undefined = $state(undefined);

	// Player options for combobox
	const playerOptions = $derived(
		Object.entries(appState.players).map(([id, player]) => ({
			label: (player as Player).nickname || `Player ${id}`,
			value: id
		}))
	);

	// Auto-select current player if available
	$effect(() => {
		if (appState.selectedPlayer && !selectedPlayerId) {
			selectedPlayerId = appState.selectedPlayer.uid;
		}
	});

	// Check if target is available
	const isTargetAvailable = $derived.by(() => {
		switch (exportTarget) {
			case 'pal_box':
				return selectedPlayerId && appState.players[selectedPlayerId];
			case 'dps':
				return selectedPlayerId && appState.players[selectedPlayerId]?.dps;
			case 'gps':
				return !!appState.gps;
			default:
				return false;
		}
	});

	function handleClose(confirmed: boolean) {
		if (!confirmed) {
			closeModal(null);
			return;
		}

		closeModal({
			target: exportTarget,
			playerId: exportTarget === 'gps' ? undefined : selectedPlayerId
		});
	}

	function getTargetIcon() {
		switch (exportTarget) {
			case 'pal_box':
				return Upload;
			case 'dps':
				return Download;
			case 'gps':
				return Share;
			default:
				return Upload;
		}
	}

	function getTargetDescription() {
		switch (exportTarget) {
			case 'pal_box':
				const player = selectedPlayerId ? (appState.players[selectedPlayerId] as Player) : null;
				return player ? `${player.nickname || 'Player'}'s Pal Box` : "Selected Player's Pal Box";
			case 'dps':
				const dpsPlayer = selectedPlayerId ? (appState.players[selectedPlayerId] as Player) : null;
				return dpsPlayer ? `${dpsPlayer.nickname || 'Player'}'s DPS` : "Selected Player's DPS";
			case 'gps':
				return 'Global Pal Storage (GPS)';
			default:
				return 'Unknown Target';
		}
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[400px] max-w-lg">
		<div class="mb-4 flex items-center justify-between">
			<h3 class="h3 flex items-center gap-2">
				{#if exportTarget}
					{@const IconComponent = getTargetIcon()}
					<IconComponent class="h-5 w-5" />
				{/if}
				{title}
			</h3>
		</div>

		{#if message}
			<p class="mb-4 text-sm">{message}</p>
		{/if}

		<div class="space-y-4">
			<!-- Show pal count -->
			<p class="text-surface-600 dark:text-surface-400 text-sm">
				{m.exporting_count_selected_pals({ count: pals.length, pals: m.pal({count: pals.length}) })}
			</p>

			<!-- Export Target Selection -->
			<div>
				<span class="mb-2 block text-sm font-medium">{m.export_to()}</span>
				<div class="grid grid-cols-3 gap-2">
					<label
						class="hover:bg-surface-100 dark:hover:bg-surface-800 border-surface-700 flex cursor-pointer items-center justify-center space-x-2 rounded border p-3"
						class:bg-primary-500={exportTarget === 'pal_box'}
						class:text-white={exportTarget === 'pal_box'}
					>
						<input type="radio" bind:group={exportTarget} value="pal_box" class="sr-only" />
						<Upload class="h-4 w-4" />
						<span class="text-sm">{m.palbox()}</span>
					</label>
					<label
						class="hover:bg-surface-100 dark:hover:bg-surface-800 border-surface-700 flex cursor-pointer items-center justify-center space-x-2 rounded border p-3"
						class:bg-primary-500={exportTarget === 'dps'}
						class:text-white={exportTarget === 'dps'}
					>
						<input type="radio" bind:group={exportTarget} value="dps" class="sr-only" />
						<Download class="h-4 w-4" />
						<span class="text-sm">{m.dps()}</span>
					</label>
					<label
						class="hover:bg-surface-100 dark:hover:bg-surface-800 border-surface-700 flex cursor-pointer items-center justify-center space-x-2 rounded border p-3"
						class:bg-primary-500={exportTarget === 'gps'}
						class:text-white={exportTarget === 'gps'}
					>
						<input type="radio" bind:group={exportTarget} value="gps" class="sr-only" />
						<Share class="h-4 w-4" />
						<span class="text-sm">{m.gps()}</span>
					</label>
				</div>
			</div>

			<!-- Player Selection (for Pal Box and DPS) -->
			{#if exportTarget === 'pal_box' || exportTarget === 'dps'}
				<div>
					<span class="mb-2 block text-sm font-medium">{m.select_entity({ entity: m.player({ count: 1 }) })}</span>
					<Combobox
						bind:value={selectedPlayerId}
						options={playerOptions}
						placeholder={m.choose_a_entity({ entity: c.player })}
						inputClass="w-full"
					/>
				</div>
			{/if}

			<!-- Target description and availability check -->
			<div class="bg-surface-100 dark:bg-surface-800 rounded p-3 text-sm">
				<p class="mb-1 font-medium">{m.export_target()}:</p>
				<p class="text-surface-600 dark:text-surface-400 mb-2">
					{getTargetDescription()}
				</p>

				{#if !isTargetAvailable}
					<div class="text-red-600 dark:text-red-400">
						{#if exportTarget === 'pal_box'}
							⚠ {m.select_valid_player()}
						{:else if exportTarget === 'dps'}
							⚠ {m.dps_not_available()}
						{:else if exportTarget === 'gps'}
							⚠ {m.gps_not_available()}
						{/if}
					</div>
				{:else}
					<div class="text-green-600 dark:text-green-400">✓ {m.target_available()}</div>
				{/if}
			</div>

			<!-- Warning about export -->
			<div class="rounded bg-yellow-100 p-3 text-sm dark:bg-yellow-900/20">
				<p class="mb-1 font-medium text-yellow-800 dark:text-yellow-200">{m.note()}:</p>
				<p class="text-yellow-700 dark:text-yellow-300">
					{m.export_note_message()}
				</p>
			</div>
		</div>

		<!-- Actions -->
		<div class="mt-6 flex justify-end gap-2">
			<button
				type="button"
				onclick={() => handleClose(false)}
				class="bg-surface-500 hover:bg-surface-600 flex items-center gap-2 rounded-md px-4 py-2 text-white"
			>
				<X class="h-4 w-4" />
				{m.cancel()}
			</button>
			<button
				type="button"
				onclick={() => handleClose(true)}
				class="bg-primary-500 hover:bg-primary-600 flex items-center gap-2 rounded-md px-4 py-2 text-white"
				data-modal-primary
				disabled={!isTargetAvailable}
			>
				<Save class="h-4 w-4" />
				{m.export_to_target({ target: exportTarget.toUpperCase() })}
			</button>
		</div>
	</Card>
</div>
