<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Tooltip } from '$components/ui';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { elementsData } from '$lib/data';
	import { cn } from '$theme';
	import { staticIcons } from '$types/icons';
	import { assetLoader, calculateFilters } from '$utils';
	import * as m from '$i18n/messages';
	import { c } from '$utils/commonTranslations';

	let { selectedFilter = $bindable() }: { selectedFilter: string } = $props();

	const elementTypes = $derived(Object.keys(elementsData.elements));
	const elementIcons = $derived.by(() => {
		let icons: Record<string, string> = {};
		for (const element of elementTypes) {
			const elementData = elementsData.elements[element];
			if (elementData) {
				icons[element] = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${elementData.icon}.webp`
				) as string;
			}
		}
		return icons;
	});

	const filterClass = (filter: string) =>
		cn('btn', selectedFilter === filter ? 'bg-secondary-500/25' : '');
</script>

<div>
	<legend class="font-bold">{m.element_and_type()}</legend>
	<hr />
	<div class="mt-2 grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 2xl:grid-cols-6">
		<Tooltip>
			<button class={filterClass('All')} onclick={() => (selectedFilter = 'All')}>
				<Icon icon="tabler:layout-list" />
			</button>
			{#snippet popup()}{m.all_entity({ entity: c.pals })}{/snippet}
		</Tooltip>
		{#each [...elementTypes] as element}
			{@const localizedName = elementsData.getByKey(element)?.localized_name}
			<Tooltip label={localizedName}>
				<button
					class={filterClass(element)}
					onclick={() => (selectedFilter = element)}
					aria-label={localizedName}
				>
					<img src={elementIcons[element]} alt={localizedName} class="pal-element-badge" />
				</button>
			</Tooltip>
		{/each}
		<Tooltip label={c.alphaPals}>
			<button type="button" class={filterClass('alpha')} onclick={() => (selectedFilter = 'alpha')}>
				<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge" />
			</button>
		</Tooltip>
		<Tooltip label={c.luckyPals}>
			<button type="button" class={filterClass('lucky')} onclick={() => (selectedFilter = 'lucky')}>
				<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge" />
			</button>
		</Tooltip>
		<Tooltip label={m.awakened()}>
			<button
				type="button"
				class={filterClass('awakened')}
				onclick={() => (selectedFilter = 'awakened')}
			>
				<img src={staticIcons.awakeningIcon} alt="Awakened" class="pal-element-badge" />
			</button>
		</Tooltip>
		<Tooltip label={m.imported()}>
			<button
				type="button"
				class={filterClass('imported')}
				onclick={() => (selectedFilter = 'imported')}
			>
				<img src={staticIcons.importedIcon} alt="Imported" class="pal-element-badge" />
			</button>
		</Tooltip>
		<Tooltip label={c.humans}>
			<button type="button" class={filterClass('human')} onclick={() => (selectedFilter = 'human')}>
				<Icon icon="tabler:user" />
			</button>
		</Tooltip>
		<Tooltip label={c.predatorPals}>
			<button
				type="button"
				class={filterClass('predator')}
				onclick={() => (selectedFilter = 'predator')}
			>
				<img
					src={staticIcons.predatorIcon}
					alt="Predator"
					class="pal-element-badge"
					style="filter: {calculateFilters('#FF0000')};"
				/>
			</button>
		</Tooltip>
		<Tooltip label={c.oilRigPals}>
			<button
				type="button"
				class={filterClass('oilrig')}
				onclick={() => (selectedFilter = 'oilrig')}
			>
				<img src={staticIcons.oilrigIcon} alt="Oil Rig" class="pal-element-badge" />
			</button>
		</Tooltip>
		<Tooltip label={c.summonedPals}>
			<button
				type="button"
				class={filterClass('summon')}
				onclick={() => (selectedFilter = 'summon')}
			>
				<img src={staticIcons.altarIcon} alt="Summoned" class="pal-element-badge" />
			</button>
		</Tooltip>
	</div>
</div>

<style>
	.pal-element-badge {
		width: 24px;
		height: 24px;
	}
</style>
