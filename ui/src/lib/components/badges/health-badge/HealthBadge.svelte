<script lang="ts">
	import { staticIcons } from '$lib/constants';
	import { EntryState, type Pal, type Player } from '$types';
	import { Tooltip, Progress } from '$components/ui';
	import { palsData } from '$lib/data';

	let {
		pal = $bindable(),
		player = $bindable(),
		width = 'w-[400px]',
		showActions = true,
		showStomachLabel = true,
		healthHeight = 'h-6',
		stomachHeight = 'h-6'
	}: {
		pal: Pal | undefined;
		player: Player | undefined;
		width?: string;
		showActions?: boolean;
		showStomachLabel?: boolean;
		healthHeight?: string;
		stomachHeight?: string;
	} = $props();

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

	let maxStomach = $derived.by(() => {
		if (pal && player) {
			const palData = palsData.pals[pal.character_key] || undefined;
			if (palData) {
				return palData.max_full_stomach;
			}
		}
		return 150;
	});
</script>

{#if pal}
	<div class="flex flex-row items-center">
		{#if showActions}
			<Tooltip>
				<button onclick={handleHeal} aria-label="Health">
					<img src={staticIcons.hpIcon} alt="Health" class="mr-2 h-6 w-6" />
				</button>

				{#snippet popup()}
					<span>HP</span>
					{Math.round(pal.hp / 1000)}/{pal.max_hp / 1000}
				{/snippet}
			</Tooltip>
		{/if}
		<Progress
			bind:value={pal.hp}
			bind:max={pal.max_hp}
			height={healthHeight}
			color="green"
			{width}
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
			{width}
			color="orange"
			showLabel={showStomachLabel}
		/>
	</div>
	{#if showActions}
		<div
			class="bg-surface-800 text-one-surface hover:ring-secondary-500 relative mx-2 flex h-1/2 rounded px-6 py-1 font-semibold hover:ring"
		>
			<span class="relative z-10 grow">SAN</span>
			<div class="relative z-10 flex space-x-0.5">
				<span class="font-bold">{pal.sanity.toFixed(0)}</span>
				<span class="text-surface-400 text-sm">/ 100</span>
			</div>
			<span class="border-surface-700 absolute inset-0 rounded border"></span>
			<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
			<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
			<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
			<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
		</div>
	{/if}
{/if}
