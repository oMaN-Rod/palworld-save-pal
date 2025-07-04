<script lang="ts">
	import { staticIcons } from '$types/icons';
	import { EntryState, type Pal, type Player } from '$types';
	import { Tooltip, Progress } from '$components/ui';
	import { palsData } from '$lib/data';
	import { friendshipData } from '$lib/data/friendship.svelte';

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
			const palData = palsData.pals[pal.character_key] || undefined;
			if (palData) {
				return palData.max_full_stomach;
			}
		}
		return 150;
	});

	// Friendship data: use synchronously if loaded, otherwise trigger load
	let trustLevel = $state(0);
	let trustCurrent = $state(0);
	let trustMax = $state(0);

	const friendship = friendshipData.friendshipData;
	if (pal) {
		const palTrust = pal.friendship_point ?? 0;
		let currentLevel = 0;
		let currentMax = 0;
		let prevRequired = 0;
		// If not loaded, trigger load (async, will update on next render)
		if (Object.keys(friendship).length === 0) {
			friendshipData.getFriendshipData();
		} else {
			for (const [levelStr, { rank, required_point }] of Object.entries(friendship)) {
				if (palTrust >= required_point) {
					currentLevel = rank;
					prevRequired = required_point;
				} else {
					currentMax = required_point - prevRequired;
					break;
				}
			}
			trustLevel = currentLevel;
			trustCurrent = pal.friendship_point - prevRequired;
			trustMax = currentMax;
		}
	}

	function handleHeal() {
		if (!pal) return;
		pal.hp = pal.max_hp;
		pal.state = EntryState.MODIFIED;
	}

	async function handleEat() {
		if (!pal) return;
		const palData = palsData.pals[pal.character_key] || undefined;
		if (!palData) return;
		pal.stomach = palData.max_full_stomach;
		pal.state = EntryState.MODIFIED;
	}
</script>

{#if pal}
	<div class="mb-2 flex items-center">
		<img src={staticIcons.hpIcon} alt="Trust" class="mr-2 h-6 w-6" />
		<Progress value={trustCurrent} max={trustMax} height={healthHeight} color="bg-[#db7c90]" />
		<div class="absolute right-8 flex items-center">
			<span class="text-xs font-bold text-white">Lv.{trustLevel}</span>
		</div>
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
