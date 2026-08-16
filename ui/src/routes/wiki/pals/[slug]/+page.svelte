<script lang="ts">
	import { palsData, elementsData, activeSkillsData } from '$lib/data';
	import { WikiEntity } from '$components/docs';
	import { PalModelViewer } from '$components/pal';
	import { Loading } from '$components/ui';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import {
		assetLoader,
		getWorkSuitabilityFormattedName,
		suitabilityImageMap,
		wazaIdFromStr
	} from '$utils';
	import { c } from '$lib/utils/commonTranslations';
	import { entityLink } from '$lib/utils/wikiCategories';
	import { buildSlugIndex, keyFromSlug } from '$lib/utils/wikiSlug';
	import type { PalData, WorkSuitability } from '$types';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';

	let { data }: { data: { slug: string } } = $props();

	const palKeys = $derived(Object.keys(palsData.pals));
	const hasData = $derived(palKeys.length > 0);
	const slugIndex = $derived(buildSlugIndex(palKeys));
	const palKey = $derived(keyFromSlug(data.slug, slugIndex));
	const pal = $derived(palKey ? palsData.pals[palKey] : undefined);

	function getElementIcon(element: string): string {
		const el = elementsData.elements[element];
		if (!el) return '';
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${el.icon}.webp`) as string;
	}

	function getWorkSuitabilities(p: PalData): [string, string, number][] {
		return Object.entries(p.work_suitability)
			.filter(([, val]) => val > 0)
			.map(([key, val]) => [key, getWorkSuitabilityFormattedName(key as WorkSuitability), val]);
	}

	const workSuits = $derived(pal ? getWorkSuitabilities(pal) : []);
</script>

<svelte:head>
	<title>{pal ? pal.localized_name : data.slug}</title>
</svelte:head>

{#snippet elementIcon(element: string, size: string)}
	{@const icon = getElementIcon(element)}
	{#if icon}
		{@const link = entityLink('elements', element)}
		<a href={link.href}><img src={icon} alt={element} class={size} /></a>
	{/if}
{/snippet}

{#snippet palImageFallback()}
	<img
		src={assetLoader.loadPalImage(palKey ?? '', pal?.is_pal ?? true)}
		alt={`${pal?.localized_name} icon`}
		class="size-full object-contain"
	/>
{/snippet}

{#if pal && palKey}
	<WikiEntity category="pals" title={pal.localized_name} subtitle={palKey} breadcrumbLabel={c.pal}>
		{#snippet icons()}
			{#each pal.element_types as element (element)}
				{@render elementIcon(element, 'h-6 w-6')}
			{/each}
			<span class="text-surface-400 text-sm">#{pal.pal_deck_index}</span>
		{/snippet}

		{#snippet media()}
			<!-- Pals with no baked mesh (NPCs and a handful of variants) render the
			     deck artwork instead, so the panel keeps its size either way. -->
			<div class="relative aspect-square w-full">
				<PalModelViewer characterKey={palKey} fallback={palImageFallback} />
			</div>
		{/snippet}

		{#snippet infobox()}
			<p class="text-surface-300">{pal.description}</p>

			<div class="mt-4 grid grid-cols-3 gap-4">
				<div class="bg-surface-900 rounded-md p-3">
					<div class="flex gap-2">
						<img src={staticIcons.hpIcon} alt="HP icon" class="h-4 w-4" />
						<span class="text-surface-500 text-xs">HP</span>
					</div>
					<p class="text-lg font-semibold">{pal.scaling.hp}</p>
				</div>
				<div class="bg-surface-900 rounded-md p-3">
					<div class="flex gap-2">
						<img src={staticIcons.attackIcon} alt="Attack icon" class="h-4 w-4" />
						<span class="text-surface-500 text-xs">Attack</span>
					</div>
					<p class="text-lg font-semibold">{pal.scaling.attack}</p>
				</div>
				<div class="bg-surface-900 rounded-md p-3">
					<div class="flex gap-2">
						<img src={staticIcons.defenseIcon} alt="Defense icon" class="h-4 w-4" />
						<span class="text-surface-500 text-xs">Defense</span>
					</div>
					<p class="text-lg font-semibold">{pal.scaling.defense}</p>
				</div>
			</div>

			<div class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-4">
				<div>
					<span class="text-surface-500 text-xs">Size</span>
					<p class="text-sm">{pal.size}</p>
				</div>
				<div>
					<span class="text-surface-500 text-xs">Rarity</span>
					<p class="text-sm">{pal.rarity}</p>
				</div>
				<div>
					<span class="text-surface-500 text-xs">Food</span>
					<p class="text-sm">{pal.food_amount}</p>
				</div>
				<div>
					<span class="text-surface-500 text-xs">Stamina</span>
					<p class="text-sm">{pal.stamina}</p>
				</div>
				<div>
					<span class="text-surface-500 text-xs">Walk Speed</span>
					<p class="text-sm">{pal.walk_speed}</p>
				</div>
				<div>
					<span class="text-surface-500 text-xs">Run Speed</span>
					<p class="text-sm">{pal.run_speed}</p>
				</div>
				<div>
					<span class="text-surface-500 text-xs">Ride Sprint</span>
					<p class="text-sm">{pal.ride_sprint_speed}</p>
				</div>
				<div>
					<span class="text-surface-500 text-xs">Capture Rate</span>
					<p class="text-sm">{pal.capture_rate_correct}</p>
				</div>
			</div>
		{/snippet}

		{#if workSuits.length > 0}
			<div class="mt-5">
				<h3 class="text-surface-400 mb-2 text-sm font-semibold">Work Suitability</h3>
				<div class="flex flex-wrap gap-2">
					{#each workSuits as [key, type, level] (key)}
						{@const iconPath = assetLoader.loadImage(
							`${ASSET_DATA_PATH}/img/${suitabilityImageMap[key as WorkSuitability]}.webp`
						)}
						<div class="bg-surface-900 flex items-center gap-2 rounded-md px-3 py-1 text-sm">
							<img src={iconPath} alt="{type} icon" class="h-4 w-4 2xl:h-6 2xl:w-6" />
							<div>
								{type} <span class="text-surface-400 font-semibold">Lv.{level}</span>
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		{#if pal.passive_skills.length > 0}
			<div class="mt-5">
				<h3 class="text-surface-400 mb-2 text-sm font-semibold">Partner Skills</h3>
				<div class="flex flex-wrap gap-2">
					{#each pal.passive_skills as skill (skill)}
						{@const link = entityLink('passive-skills', skill)}
						<a
							href={link.href}
							class="bg-surface-900 hover:bg-surface-800 rounded-md px-3 py-1 text-sm"
						>
							{skill}
						</a>
					{/each}
				</div>
			</div>
		{/if}

		{#if pal.skill_set && Object.keys(pal.skill_set).length > 0}
			<div class="mt-5">
				<h3 class="text-surface-400 mb-2 text-sm font-semibold">Skill Set</h3>
				<div class="flex flex-wrap gap-2">
					{#each Object.entries(pal.skill_set) as [skill, level] (skill)}
						{@const [, skillId] = wazaIdFromStr(`EPalWazaID::${skill}`)}
						{@const skillData = activeSkillsData.getByKey(skillId)}
						{@const skillElement = skillData?.details?.element}
						{@const link = entityLink('active-skills', skillId)}
						<div class="bg-surface-900 flex items-center gap-2 rounded-md px-3 py-1 text-sm">
							{#if skillElement}
								{@render elementIcon(skillElement, 'h-4 w-4')}
							{/if}
							<a href={link.href}>{skillData?.localized_name || skill}</a>
							<span class="text-surface-400">Lv.{level}</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	</WikiEntity>
{:else if hasData}
	<div class="text-surface-400 flex items-center justify-center py-12">
		<p>{m.docs_no_results()}</p>
	</div>
{:else}
	<Loading label={m.loading_entity({ entity: c.pal })} />
{/if}
