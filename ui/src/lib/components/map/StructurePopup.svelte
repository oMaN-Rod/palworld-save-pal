<script lang="ts">
	import { Card } from '$components/ui';
	import type { BaseStructure } from '$types';
	import { Heart, Ruler, Award, User } from 'lucide-svelte';
	import { structureInfo } from './structureInfo';
	import { structureColors } from './mapColors.svelte';
	import { baseStructuresData, buildingsData } from '$lib/data';
	import { getAppState } from '$states';

	let { structure }: { structure: BaseStructure } = $props();

	const appState = getAppState();
	const info = $derived(
		structureInfo(
			structure,
			baseStructuresData.footprints,
			buildingsData.buildings,
			appState.playerSummaries
		)
	);
	const swatchColor = $derived.by(() => {
		const colors = structureColors();
		return colors[info.typeA] ?? colors.Other;
	});
</script>

<Card class="min-w-70 shadow-lg">
	<div class="pointer-events-auto space-y-3">
		<div class="border-b pb-3">
			<div class="flex items-start gap-2">
				<span
					class="mt-1.5 h-3 w-3 shrink-0 rounded-full"
					style="background-color: {swatchColor}"
				></span>
				<div class="min-w-0 flex-1">
					<h3 class="text-foreground truncate text-lg font-bold">{info.name}</h3>
					<span class="truncate text-xs font-light">{info.typeA}</span>
					<span class="truncate text-xs font-light">{structure.map_object_id}</span>
				</div>
			</div>
			{#if info.description}
				<p class="mt-2 text-xs">{info.description}</p>
			{/if}
		</div>

		<div class="space-y-2">
			<div class="flex items-start gap-2">
				<Heart class="mt-0.5 h-4 w-4 shrink-0 text-red-500" />
				<div class="min-w-0 flex-1">
					<div class="mb-1 text-xs font-medium">HP</div>
					<div class="text-foreground font-mono text-xs">{info.hp} / {info.hpMax}</div>
				</div>
			</div>
			<div class="flex items-start gap-2">
				<Ruler class="text-primary mt-0.5 h-4 w-4 shrink-0" />
				<div class="min-w-0 flex-1">
					<div class="mb-1 text-xs font-medium">Dimensions</div>
					<div class="text-foreground font-mono text-xs">
						{info.sizeM.x.toFixed(2)} x {info.sizeM.y.toFixed(2)} x {info.sizeM.z.toFixed(2)} m
					</div>
				</div>
			</div>
			{#if info.rank !== undefined}
				<div class="flex items-start gap-2">
					<Award class="text-primary mt-0.5 h-4 w-4 shrink-0" />
					<div class="min-w-0 flex-1">
						<div class="mb-1 text-xs font-medium">Rank</div>
						<div class="text-foreground font-mono text-xs">{info.rank}</div>
					</div>
				</div>
			{/if}
			{#if info.builder}
				<div class="flex items-start gap-2">
					<User class="text-primary mt-0.5 h-4 w-4 shrink-0" />
					<div class="min-w-0 flex-1">
						<div class="mb-1 text-xs font-medium">Builder</div>
						<div class="text-foreground font-mono text-xs">{info.builder}</div>
					</div>
				</div>
			{/if}
		</div>
	</div>
</Card>
