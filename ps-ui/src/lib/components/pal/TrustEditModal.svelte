<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Slider, Tooltip } from '$components/ui';
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
		<div class="flex items-center justify-between">
			<span class="text-surface-50/80 text-sm">{m.friendship_rank()}</span>
			<span
				class="bg-tertiary-500 text-tertiary-contrast-500 rounded-full px-3 py-1 text-sm font-bold"
			>
				Lv.{currentRank}
			</span>
		</div>

		<div>
			<span class="text-surface-50/80 mb-1 block text-sm font-medium">{m.trust_xp()}</span>
			<Slider
				value={currentTrust}
				min={minTrust}
				max={maxTrust}
				color="tertiary"
				showSteppers={false}
				label={m.trust_xp()}
				onchange={updateTrust}
			/>
			<div class="text-surface-50/70 mt-2 flex justify-between text-sm">
				<span>{minTrust}</span>
				<span>{maxTrust}</span>
			</div>
		</div>
	</div>

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
