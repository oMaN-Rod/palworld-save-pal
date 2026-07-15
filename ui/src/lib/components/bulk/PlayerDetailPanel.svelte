<script lang="ts">
	import { List, Loading, Spinner, Tooltip } from '$components/ui';
	import { getAppState, getPalEditorState } from '$states';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { X } from 'lucide-svelte';
	import { Pencil, User } from '@lucide/svelte';
	import { assetLoader, calculateFilters } from '$utils';
	import { cn } from '$theme';
	import { staticIcons } from '$types/icons';
	import { PalGender } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { PalInfoPopup } from '$components/pal';
	import { palsData } from '$lib/data';
	import { PlayerHealthBadge, PlayerStats } from '$components/player';

	let { expanded = false, onclose }: { expanded?: boolean; onclose?: () => void } = $props();

	const appState = getAppState();
	const palEditor = getPalEditorState();
	const player = $derived(appState.bulkDetailPlayer);
	const maxHp = $derived(player ? 500 + player.status_point_list.max_hp * 100 : 0);

	function editPal(palId: string) {
		const targetPal = player?.pals?.[palId];
		if (!targetPal) return;
		palEditor.open(targetPal);
	}
</script>

<div
	class="bg-surface-800/80 text-on-surface h-[calc(100vh-84px)] shrink-0 overflow-hidden shadow-lg backdrop-blur-md transition-all duration-300 ease-in-out"
	style:width={expanded ? '420px' : '0px'}
>
	<div class="flex h-full w-105 flex-col overflow-y-auto p-4">
		<div class="mb-3 flex items-center justify-between">
			<span class="font-semibold">{c.player}</span>
			<button
				class="hover:text-primary-500 rounded p-1"
				onclick={() => onclose?.()}
				aria-label={m.close_drawer()}
			>
				<X class="h-4 w-4" />
			</button>
		</div>
		{#if appState.loadingPlayer}
			<div class="flex flex-1 items-center justify-center">
				<Loading label={m.loading_entity({ entity: m.player({count: 1}) })} loadingComplete={!appState.loadingPlayer} icon={User}/>
			</div>
		{:else if player}
			<div class="flex flex-col gap-3">
				<h3 class="h4">{player.nickname}</h3>
				<PlayerHealthBadge {player} {maxHp} />
				<PlayerStats {player} />
				{#if player.pals}
					<div class="flex flex-col gap-1">
						<div class="flex items-center gap-1">
							<h4 class="text-sm font-semibold">{c.pals}</h4>
							<span class="text-xs font-semibold">
								({player.pals ? Object.keys(player.pals).length : 0})
							</span>
						</div>
						<List
							items={Object.values(player.pals)}
							idKey="instance_id"
							baseClass="max-h-[435px]"
							canSelect={false}
						>
							{#snippet listItem(pal)}
								{@const palIcon = assetLoader.loadMenuImage(pal.character_id as string)}
								{@const genderIcon = assetLoader.loadImage(
									`${ASSET_DATA_PATH}/img/${pal.gender}.webp`
								)}
								{@const palData = palsData.getByKey(pal.character_key)}
								<div class="ml-2 flex gap-4">
									<div
										class={cn('relative flex items-center justify-center', {
											'animate-pulse rounded-full ring-4 ring-red-500': pal && pal.is_sick
										})}
									>
										{#if pal.is_boss}
											<div class="absolute -top-1 -left-4 h-2 w-2 xl:h-4 xl:w-4">
												<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge" />
											</div>
										{/if}
										{#if pal.is_predator}
											<div class="absolute -top-1 -left-4 h-2 w-2 xl:h-4 xl:w-4">
												<img
													src={staticIcons.predatorIcon}
													alt="Alpha"
													class="pal-element-badge"
													style="filter: {calculateFilters('#FF0000')};"
												/>
											</div>
										{/if}
										{#if pal.is_lucky}
											<div class="absolute -top-1 -left-4 h-2 w-2 xl:h-4 xl:w-4">
												<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge" />
											</div>
										{/if}
										<img src={palIcon} alt={pal.name} class="h-6 w-6 rounded-full xl:h-8 xl:w-8" />

										<div class="absolute -top-1 -right-7 h-6 w-6 xl:h-8 xl:w-8">
											<img src={genderIcon} alt={pal.gender} class="h-2 w-2 xl:h-4 xl:w-4" />
										</div>
										{#if pal.level}
											<div class="absolute -bottom-4 -left-3 h-6 w-6 xl:h-8 xl:w-8">
												<span class="text-xs font-bold">L{pal.level}</span>
											</div>
										{/if}
									</div>
									<span>{pal.nickname || palData?.localized_name || pal.character_id}</span>
								</div>
							{/snippet}
							{#snippet listItemActions(pal)}
								<button
									class="text-left text-sm hover:underline"
									onclick={() => editPal(pal.instance_id)}
								>
									<Pencil class="h-4 w-4" />
								</button>
							{/snippet}
							{#snippet listItemPopup(pal)}
								<PalInfoPopup {pal} />
							{/snippet}
						</List>
					</div>
				{/if}
			</div>
		{:else}
			<div class="flex flex-1 items-center justify-center">
				<p class="text-surface-400 text-sm">
					{m.failed_load_entity({ entity: c.player })}
				</p>
			</div>
		{/if}
	</div>
</div>
