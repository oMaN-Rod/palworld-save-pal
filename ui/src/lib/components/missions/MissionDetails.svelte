<script lang="ts">
	import { ItemBadge } from '$components';
	import { Card, SectionHeader } from '$components/ui';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import type { Mission } from '$types';
	import { assetLoader, calculateFilters } from '$utils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let { mission }: { mission?: Mission } = $props();

	const expBackground = assetLoader.loadImage(
		`${ASSET_DATA_PATH}/img/t_prt_mission_level_base.webp`
	);
	const backgroundImage = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/bg.webp`);
</script>

{#if mission}
	<Card class="flex h-full flex-col bg-none p-0">
		<div class="flex w-full flex-col items-center">
			<div class="relative flex w-full items-center justify-center p-2">
				<div
					class="absolute inset-0"
					style="background-image: url({backgroundImage}); opacity: 20%;"
				></div>
				<div class="flex w-full flex-col items-start">
					<h3 class="h3 relative">{mission.localized_name}</h3>
					<span class="text-surface-200 relative text-sm uppercase"
						>{mission.quest_type} {m.mission()}</span
					>
				</div>
			</div>
		</div>

		<div class="flex-1 overflow-y-auto p-2">
			<p class="text-surface-300 whitespace-pre-wrap text-sm leading-relaxed">
				{mission.description}
			</p>
		</div>

		{#if mission.rewards && (mission.rewards.exp || (mission.rewards.items && mission.rewards.items.length > 0))}
			<div class="flex flex-col space-y-2 p-2">
				<SectionHeader text={m.rewards()} borderClass="bg-surface-800 z-0" />
				<div class="flex flex-col space-y-2">
					{#if mission.rewards.exp}
						<div class="flex items-center gap-2 rounded p-2">
							<div class="flex flex-col items-center">
								<div class="relative flex h-12 w-12 items-center justify-center">
									<div
										class="absolute inset-0"
										style="background-image: url({expBackground}); background-size: 100% 100%; filter: {calculateFilters(
											'#2d717e'
										)}"
									></div>
									<span class="relative z-10 text-xl font-bold text-[#c1f2fc]">EXP</span>
								</div>
							</div>
							<span class="text-xl font-bold text-[#c1f2fc]">
								+{mission.rewards.exp.toLocaleString()}
							</span>
						</div>
					{/if}
					{#if mission.rewards.items && mission.rewards.items.length > 0}
						<div class="flex gap-2">
							{#each mission.rewards.items as item}
								{@const slot = { static_id: item.id, count: item.count, slot_index: 0 }}
								<ItemBadge {slot} itemGroup="Common" disabled />
							{/each}
						</div>
					{/if}
				</div>
			</div>
		{/if}
	</Card>
{:else}
	<div class=" text-surface-400 flex h-full items-center justify-center p-4">
		<p>{m.select_a_mission_to_view()}</p>
	</div>
{/if}
