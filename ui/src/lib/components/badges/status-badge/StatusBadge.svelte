<script lang="ts">
	import { EntryState, type Pal } from '$types';
	import { Tooltip, Progress } from '$components/ui';
	import { staticIcons } from '$types/icons';
	import { palsData } from '$lib/data';
	import { friendshipData } from '$lib/data';
	import { getModalState } from '$states';
	import TrustEditModal from './TrustEditModal.svelte';

	let {
		pal = $bindable(),
		showActions = true,
		showStomachLabel = true,
		healthHeight = 'h-6',
		stomachHeight = 'h-6'
	}: {
		pal: Pal | undefined;
		showActions?: boolean;
		showStomachLabel?: boolean;
		healthHeight?: string;
		stomachHeight?: string;
	} = $props();

	const palMaxHp = $derived(pal ? pal.max_hp : 1000);

	const maxStomach = $derived.by(() => {
		if (pal) {
			const palData = palsData.getPalData(pal.character_key);
			if (palData) {
				return palData.max_full_stomach;
			}
		}
		return 150;
	});

	const modal = getModalState();

	const levels = Object.values(friendshipData.friendshipData).sort((a, b) => b.rank - a.rank);
	const levelsAsc = Object.values(friendshipData.friendshipData).sort((a, b) => a.rank - b.rank);
	const trustCurrent = $derived(pal?.friendship_point ?? 0);
	const currentLevel = $derived.by(() => {
		if (!pal) return 0;
		const level = levels.find((l) => trustCurrent >= l.required_point)?.rank ?? 0;
		return level;
	});
	const nextRequired = $derived.by(() => {
		if (!pal) return 0;
		const next = levelsAsc.find((l) => trustCurrent < l.required_point);
		return next?.required_point ?? 200000; // Max level reached
	});

	function handleHeal() {
		if (!pal) return;
		pal.hp = pal.max_hp;
		pal.state = EntryState.MODIFIED;
	}

	async function handleEat() {
		if (!pal) return;
		const palData = palsData.getPalData(pal.character_key);
		if (!palData) return;
		pal.stomach = palData.max_full_stomach;
		pal.state = EntryState.MODIFIED;
	}

	async function showTrustEditModal() {
		if (!pal) return;

		// @ts-ignore
		const updatedFriendshipPoint = await modal.showModal<number>(TrustEditModal, {
			pal
		});

		if (updatedFriendshipPoint) {
			pal.friendship_point = updatedFriendshipPoint;
			pal.state = EntryState.MODIFIED;
		}
	}
</script>

{#if pal}
	<div class="mb-2 flex items-center">
		{#if showActions}
			<Tooltip label="Edit Trust Level">
				<button type="button" class="mr-2" onclick={showTrustEditModal} aria-label="Edit Trust">
					<img src={staticIcons.trustIcon} alt="Trust" class="h-6 w-6" />
				</button>
			</Tooltip>
		{/if}
		<Progress
			value={trustCurrent}
			max={nextRequired}
			height={healthHeight}
			trailingLabel={`Lv.${currentLevel}`}
			color="bg-[#db7c90]"
		/>
	</div>
	<div class="flex items-center">
		{#if showActions}
			<Tooltip>
				<button onclick={handleHeal} aria-label="Health">
					<img src={staticIcons.hpIcon} alt="Health" class="mr-2 h-6 w-6" />
				</button>
				{#snippet popup()}
					<span>HP</span>
					{Math.round(pal.hp / 1000)}/{palMaxHp / 1000}
				{/snippet}
			</Tooltip>
		{/if}
		<Progress
			bind:value={pal.hp}
			max={palMaxHp}
			height={healthHeight}
			color="green"
			dividend={1000}
		/>
	</div>
	<div class="flex flex-row items-center">
		{#if showActions}
			<Tooltip>
				<button class="mr-2" onclick={handleEat} aria-label="Food">
					<img src={staticIcons.foodIcon} alt="Food" class="h-6 w-6" />
				</button>
				{#snippet popup()}
					<span>Feed</span>
					{Math.round(pal.stomach)}/{maxStomach}
				{/snippet}
			</Tooltip>
		{/if}
		<Progress
			bind:value={pal.stomach}
			max={maxStomach}
			height={stomachHeight}
			color="orange"
			showLabel={showStomachLabel}
		/>
	</div>
	{#if showActions}
		<div
			class="bg-surface-800 text-one-surface hover:ring-secondary-500 relative mx-2 flex h-1/2 rounded-sm px-6 py-1 font-semibold hover:ring"
		>
			<span class="relative z-10 grow">SAN</span>
			<div class="relative z-10 flex space-x-0.5">
				<span class="font-bold">{pal.sanity.toFixed(0)}</span>
				<span class="text-surface-400 text-sm">/ 100</span>
			</div>
			<span class="border-surface-700 absolute inset-0 rounded-sm border"></span>
			<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
			<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
			<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
			<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
		</div>
	{/if}
{/if}
