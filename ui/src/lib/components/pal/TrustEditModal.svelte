<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Tooltip } from '$components/ui';
	import { friendshipData } from '$lib/data/friendship.svelte';
	import { type Pal } from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let { pal, closeModal } = $props<{
		pal: Pal;
		closeModal: (updatedFriendshipPoint: number | null) => void;
	}>();

	let levels: { rank: number; required_point: number }[] = $state([]);
	let maxTrust: number = $state(0);
	let minTrust: number = $state(0);
	let currentTrust: number = $state(0);
	let currentRank: number = $state(0);

	function initializeTrust() {
		levels = Object.values(friendshipData.friendshipData).sort(
			(a, b) => a.required_point - b.required_point
		);

		minTrust = levels[0]?.required_point ?? 0;
		maxTrust = levels.at(-1)?.required_point ?? 100;

		currentTrust = pal?.friendship_point ?? minTrust;
		updateRank(currentTrust);
	}

	function updateTrust(newTrust: number) {
		if (!pal) return;
		currentTrust = Math.max(minTrust, Math.min(newTrust, maxTrust));
		updateRank(currentTrust);
	}

	function updateRank(trustValue: number) {
		const rank = [...levels].reverse().find((l) => trustValue >= l.required_point)?.rank ?? 0;
		currentRank = rank;
	}

	function handleSave() {
		if (!pal) return;
		if (closeModal) closeModal(currentTrust);
	}

	function handleCancel() {
		if (closeModal) closeModal(null);
	}

	initializeTrust();
</script>

<Card class="text-surface-50 min-w-[min(100vw,24rem)] rounded-xl p-6 shadow-lg">
	<h3 class="mb-6 text-lg font-semibold">{m.edit_entity({ entity: m.trust() })}</h3>

	<div class="space-y-6">
		<!-- Trust Level Display -->
		<div class="flex items-center justify-between">
			<span class="text-surface-50/80 text-sm">{m.friendship_rank()}</span>
			<span
				class="bg-tertiary-500 text-tertiary-contrast-500 rounded-full px-3 py-1 text-sm font-bold"
			>
				Lv.{currentRank}
			</span>
		</div>

		<!-- Custom Slider -->
		<div>
			<label for="trust-slider" class="text-surface-50/80 mb-1 block text-sm font-medium"
				>{m.trust_xp()}</label
			>
			<input
				id="trust-slider"
				type="range"
				min={minTrust}
				max={maxTrust}
				step="1"
				value={currentTrust}
				oninput={(e) => updateTrust(+(e.target as HTMLInputElement).value)}
			/>
			<div class="text-surface-50/70 mt-2 flex justify-between text-sm">
				<span>{minTrust}</span>
				<span class="text-surface-50 font-semibold">{currentTrust}</span>
				<span>{maxTrust}</span>
			</div>
		</div>
	</div>

	<!-- Action Buttons -->
	<div class="mt-6 flex justify-end gap-2">
		<Tooltip position="bottom">
			{#snippet children()}
				<Button variant="ghost" size="icon" class="rounded-md px-3 py-1.5" onclick={handleSave}>
					<Icon icon="tabler:device-floppy" />
				</Button>
			{/snippet}
			{#snippet popup()}
				<span>{c.save}</span>
			{/snippet}
		</Tooltip>

		<Tooltip position="bottom">
			{#snippet children()}
				<Button variant="ghost" size="icon" class="rounded-md px-3 py-1.5" onclick={handleCancel}>
					<Icon icon="tabler:x" />
				</Button>
			{/snippet}
			{#snippet popup()}
				<span>{m.cancel()}</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>

<style>
	input[type='range'] {
		appearance: none;
		-webkit-appearance: none;
		width: 100%;
		height: 6px;
		background: var(--color-surface-700);
		border-radius: 3px;
		outline: none;
	}

	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 16px;
		height: 16px;
		border-radius: 9999px;
		background: var(--color-tertiary-400);
		border: 2px solid var(--color-surface-50);
		cursor: pointer;
		transition: transform 0.2s ease;
	}

	input[type='range']::-moz-range-thumb {
		width: 16px;
		height: 16px;
		border-radius: 9999px;
		background: var(--color-tertiary-400);
		border: 2px solid var(--color-surface-50);
		cursor: pointer;
		transition: transform 0.2s ease;
	}

	input[type='range']::-webkit-slider-thumb:hover,
	input[type='range']::-moz-range-thumb:hover {
		transform: scale(1.1);
	}
</style>
