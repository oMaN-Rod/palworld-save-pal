<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { IControl } from 'maplibre-gl';
	import { untrack } from 'svelte';
	import { getMapContext, type ControlPosition } from '$components/maplibre';
	import { Accordion, Switch } from '@skeletonlabs/skeleton-svelte';
	import * as m from '$i18n/messages';
	import { debounce } from '$utils';
	import {
		MATERIAL_ORDER,
		materialBlend,
		materialOpacities,
		materialTints,
		resetMapColors,
		setMaterialBlend,
		setMaterialOpacity,
		setMaterialTint,
		setStructureColor,
		structureColors
	} from './mapColors.svelte';
	import { Button, Slider } from '$components/ui';
	import type { ValueChangeDetails as AccordionValueChangeDetails } from '@zag-js/accordion';
	import { sliderToScale as palSliderToScale, scaleToSlider as palScaleToSlider } from './palSize';
	import {
		sliderToScale as objectSliderToScale,
		scaleToSlider as objectScaleToSlider
	} from './mapObjectSize';
	import { MAP_OPACITY_MIN } from './mapOpacity';

	let {
		types,
		enabled,
		open,
		onToggleOpen,
		ontoggle,
		position = 'top-right',
		title,
		show3d,
		showStructureControls,
		detailed,
		textured,
		palAutoFollow,
		ontoggledetailed,
		ontoggletextured,
		ontogglepalautofollow,
		palSize,
		fastTravelSize,
		watchtowerSize,
		relicSize,
		palHeight,
		mapOpacity,
		onPalSizeChange,
		onFastTravelSizeChange,
		onWatchtowerSizeChange,
		onRelicSizeChange,
		onPalHeightChange,
		onMapOpacityChange
	}: {
		types: string[];
		enabled: Record<string, boolean>;
		open: boolean;
		onToggleOpen: () => void;
		ontoggle: (type: string) => void;
		position?: ControlPosition;
		title: string;
		show3d: boolean;
		showStructureControls: boolean;
		detailed: boolean;
		textured: boolean;
		palAutoFollow: boolean;
		ontoggledetailed: () => void;
		ontoggletextured: () => void;
		ontogglepalautofollow: () => void;
		palSize: number;
		fastTravelSize: number;
		watchtowerSize: number;
		relicSize: number;
		palHeight: number;
		mapOpacity: number;
		onPalSizeChange: (scale: number) => void;
		onFastTravelSizeChange: (scale: number) => void;
		onWatchtowerSizeChange: (scale: number) => void;
		onRelicSizeChange: (scale: number) => void;
		onPalHeightChange: (heightCm: number) => void;
		onMapOpacityChange: (opacity: number) => void;
	} = $props();

	const ctx = getMapContext();

	const OPTIONS_ICON =
		"url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='29' height='29' viewBox='-2.5 -2.5 29 29' fill='none' stroke='%23333' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M20 7h-9'/%3E%3Cpath d='M14 17H5'/%3E%3Ccircle cx='17' cy='17' r='3'/%3E%3Ccircle cx='7' cy='7' r='3'/%3E%3C/svg%3E\")";

	const structureSections = $derived(show3d && showStructureControls);
	const detailedSections = $derived(detailed && showStructureControls);

	const colors = $derived(structureColors());
	const tints = $derived(materialTints());
	const blend = $derived(materialBlend());
	const opacities = $derived(materialOpacities());

	let accordionValue = $state<string[]>(
		untrack(() => (showStructureControls ? ['structures'] : ['sizes']))
	);

	let blendPercent = $state(untrack(() => Math.round(blend * 100)));
	$effect(() => {
		blendPercent = Math.round(blend * 100);
	});

	let opacityPercents = $state<Record<string, number>>(
		untrack(() =>
			Object.fromEntries(
				MATERIAL_ORDER.map((mat) => [mat, Math.round((opacities[mat] ?? 1) * 100)])
			)
		)
	);
	$effect(() => {
		opacityPercents = Object.fromEntries(
			MATERIAL_ORDER.map((mat) => [mat, Math.round((opacities[mat] ?? 1) * 100)])
		);
	});

	let pendingStructures: Record<string, string> = {};
	let pendingMaterials: Record<string, string> = {};
	let pendingOpacities: Record<string, number> = {};

	const flushStructures = debounce(() => {
		for (const [type, hex] of Object.entries(pendingStructures)) setStructureColor(type, hex);
		pendingStructures = {};
	}, 120);

	const flushMaterials = debounce(() => {
		for (const [material, hex] of Object.entries(pendingMaterials)) setMaterialTint(material, hex);
		pendingMaterials = {};
	}, 120);

	const flushOpacities = debounce(() => {
		for (const [material, value] of Object.entries(pendingOpacities)) {
			setMaterialOpacity(material, value);
		}
		pendingOpacities = {};
	}, 120);

	const flushBlend = debounce((value: number) => setMaterialBlend(value), 120);

	function queueStructureColor(type: string, hex: string) {
		pendingStructures[type] = hex;
		flushStructures();
	}

	function queueMaterialTint(material: string, hex: string) {
		pendingMaterials[material] = hex;
		flushMaterials();
	}

	function queueMaterialOpacity(material: string, percent: number) {
		opacityPercents[material] = percent;
		pendingOpacities[material] = percent / 100;
		flushOpacities();
	}

	function handleReset() {
		pendingStructures = {};
		pendingMaterials = {};
		pendingOpacities = {};
		resetMapColors();
	}

	function stopMapGesturesAction(node: HTMLDivElement) {
		const stop = (event: Event) => event.stopPropagation();
		node.addEventListener('mousedown', stop);
		node.addEventListener('dblclick', stop);
		node.addEventListener('wheel', stop);

		return {
			destroy() {
				node.removeEventListener('mousedown', stop);
				node.removeEventListener('dblclick', stop);
				node.removeEventListener('wheel', stop);
			}
		};
	}

	// Read through a holder so a new inline callback identity cannot re-create
	// the control on every parent render.
	const latest: { onToggleOpen?: () => void } = { onToggleOpen: untrack(() => onToggleOpen) };
	$effect(() => {
		latest.onToggleOpen = onToggleOpen;
	});

	let button = $state<HTMLButtonElement>();
	let icon = $state<HTMLSpanElement>();
	let panel = $state<HTMLDivElement>();

	$effect(() => {
		const map = ctx.map;
		if (!map) return;

		const container = document.createElement('div');
		container.className = 'maplibregl-ctrl maplibregl-ctrl-group map-3d-options-ctrl relative';

		const el = document.createElement('button');
		el.type = 'button';
		el.className =
			'maplibregl-ctrl-map-3d-options flex h-[29px] w-[29px] items-center justify-center';

		const iconEl = document.createElement('span');
		iconEl.className =
			'maplibregl-ctrl-icon h-4 w-4 bg-contain bg-center bg-no-repeat transition-opacity';
		iconEl.setAttribute('aria-hidden', 'true');
		iconEl.style.backgroundImage = OPTIONS_ICON;
		el.appendChild(iconEl);
		container.appendChild(el);

		const handleClick = () => latest.onToggleOpen?.();
		el.addEventListener('click', handleClick);

		const panelEl = untrack(() => panel);
		if (panelEl) container.appendChild(panelEl);
		icon = iconEl;

		const control: IControl = {
			onAdd: () => container,
			onRemove: () => {
				// Detach the Svelte-owned panel first, so its teardown finds it already
				// parentless instead of racing to remove it from a dying container.
				if (panelEl && panelEl.parentNode === container) {
					container.removeChild(panelEl);
				}
				container.remove();
			}
		};
		ctx.addControl(
			control,
			untrack(() => position)
		);
		button = el;

		return () => {
			el.removeEventListener('click', handleClick);
			button = undefined;
			icon = undefined;
			ctx.removeControl(control);
		};
	});

	$effect(() => {
		const el = button;
		if (!el) return;
		el.title = title;
		el.setAttribute('aria-label', title);
		el.setAttribute('aria-expanded', String(open));
		el.classList.toggle('is-active', open);

		const iconEl = icon;
		if (iconEl) {
			iconEl.classList.toggle('opacity-25', !open);
		}
	});
</script>

{#snippet toggleRow(label: string, checked: boolean, onchange: () => void)}
	<label
		class="border-surface-800 flex items-center justify-between gap-1.5 border-b py-1.5 text-xs whitespace-nowrap"
	>
		<span>{label}</span>
		<Switch {checked} onCheckedChange={() => onchange()} />
	</label>
{/snippet}

{#snippet sizeRow(
	label: string,
	percent: number,
	readout: string,
	onpercent: (percent: number) => void,
	min = 0,
	max = 100
)}
	<span>{label}</span>
	<Slider size="xs" {min} {max} {label} value={percent} onchange={onpercent} />
	<span class="w-9.5 shrink-0 text-right tabular-nums">{readout}</span>
{/snippet}

<div
	bind:this={panel}
	use:stopMapGesturesAction
	class={[
		'absolute top-full right-0 z-10 mt-1 max-h-[min(80vh,620px)] w-[min(20rem,calc(100vw-1rem))] overflow-y-auto rounded bg-surface-900/95 p-2 text-(--svlibre-ctrl-color,#333) shadow-(--svlibre-ctrl-shadow,0_0_0_2px_rgba(0,0,0,0.1))',
		open ? 'flex flex-col gap-1.5' : 'hidden'
	]}
	role="group"
	aria-label={title}
>
	{#if structureSections}
		{@render toggleRow(m.detailed_structures(), detailed, ontoggledetailed)}
	{/if}
	{#if detailedSections}
		{@render toggleRow(m.textured_structures(), textured, ontoggletextured)}
	{/if}
	{#if show3d}
		{@render toggleRow(m.pal_auto_follow(), palAutoFollow, ontogglepalautofollow)}
	{/if}
	<Accordion
		value={accordionValue}
		onValueChange={(e: AccordionValueChangeDetails) => (accordionValue = e.value)}
		collapsible
	>
		{#if structureSections}
			<Accordion.Item
				value="structures"
				controlBase="flex! text-start! items-center! space-x-4! w-full!"
				controlHover="hover:bg-secondary-500/50!"
				controlPadding="py-2!"
			>
				{#snippet control()}
					{m.structures()}
				{/snippet}
				{#snippet panel()}
					<div class="flex flex-col gap-1">
						{#each types as type (type)}
							<div class="flex items-center gap-1.5">
								<input
									type="color"
									class="h-4.5 w-4.5 shrink-0 cursor-pointer rounded-full bg-transparent p-0 [&::-moz-color-swatch]:rounded-xs [&::-moz-color-swatch]:border-0 [&::-webkit-color-swatch]:rounded-xs [&::-webkit-color-swatch]:border-0 [&::-webkit-color-swatch-wrapper]:p-0"
									aria-label="{type} color"
									value={colors[type]}
									oninput={(e) => queueStructureColor(type, e.currentTarget.value)}
									onchange={(e) => queueStructureColor(type, e.currentTarget.value)}
								/>
								<Switch
									checked={enabled[type] !== false}
									onCheckedChange={() => ontoggle(type)}
									compact
									classes="h-6 w-6"
								>
									<span>{type}</span>
									{#snippet inactiveChild()}<Icon icon="tabler:eye-closed" class="h-4 w-4" />{/snippet}
									{#snippet activeChild()}<Icon icon="tabler:eye" class="h-4 w-4" />{/snippet}
								</Switch>
							</div>
						{/each}
					</div>
				{/snippet}
			</Accordion.Item>
		{/if}
		{#if detailedSections}
			<Accordion.Item
				value="materials"
				controlBase="flex! text-start! items-center! space-x-4! w-full!"
				controlHover="hover:bg-secondary-500/50!"
				controlPadding="py-2!"
			>
				{#snippet control()}
					{m.materials()}
				{/snippet}
				{#snippet panel()}
					<div class="flex flex-col gap-1">
						{#each MATERIAL_ORDER as material (material)}
							<div class="flex items-center gap-1.5 text-xs whitespace-nowrap">
								<input
									type="color"
									class="h-[18px] w-[18px] shrink-0 cursor-pointer rounded-full bg-transparent p-0 [&::-moz-color-swatch]:rounded-xs [&::-moz-color-swatch]:border-0 [&::-webkit-color-swatch]:rounded-[2px] [&::-webkit-color-swatch]:border-0 [&::-webkit-color-swatch-wrapper]:p-0"
									aria-label="{material} color"
									value={tints[material]}
									oninput={(e) => queueMaterialTint(material, e.currentTarget.value)}
									onchange={(e) => queueMaterialTint(material, e.currentTarget.value)}
								/>
								<span class="w-[58px] shrink-0">{material}</span>
								<Slider
									size="xs"
									label="{material} opacity"
									value={opacityPercents[material]}
									onchange={(percent) => queueMaterialOpacity(material, percent)}
								/>
								<span class="w-[34px] shrink-0 text-right tabular-nums"
									>{opacityPercents[material]}%</span
								>
							</div>
						{/each}
					</div>
					<div class="mt-1.5 flex items-center gap-1.5 text-xs whitespace-nowrap">
						<span>{m.material_blend()}</span>
						<Slider
							size="xs"
							label={m.material_blend()}
							value={blendPercent}
							onchange={(percent) => {
								blendPercent = percent;
								flushBlend(blendPercent / 100);
							}}
						/>
						<span class="w-8.5 text-right tabular-nums">{blendPercent}%</span>
					</div>
				{/snippet}
			</Accordion.Item>
		{/if}
		<Accordion.Item
			value="sizes"
			controlBase="flex! text-start! items-center! space-x-4! w-full!"
			controlHover="hover:bg-secondary-500/50!"
			controlPadding="py-2!"
		>
			{#snippet control()}
				{m.sizes()}
			{/snippet}
			{#snippet panel()}
				<div class="grid grid-cols-[auto_1fr_auto] items-center gap-1.5">
					{#if show3d}
						{@render sizeRow(
							m.pal_size(),
							Math.round(palScaleToSlider(palSize) * 100),
							`${palSize.toFixed(1)}x`,
							(percent) => onPalSizeChange(palSliderToScale(percent / 100))
						)}
						{@render sizeRow(
							m.fast_travel_size(),
							Math.round(objectScaleToSlider(fastTravelSize) * 100),
							`${fastTravelSize.toFixed(1)}x`,
							(percent) => onFastTravelSizeChange(objectSliderToScale(percent / 100))
						)}
						{@render sizeRow(
							m.watchtower_size(),
							Math.round(objectScaleToSlider(watchtowerSize) * 100),
							`${watchtowerSize.toFixed(1)}x`,
							(percent) => onWatchtowerSizeChange(objectSliderToScale(percent / 100))
						)}
						{@render sizeRow(
							m.relic_size(),
							Math.round(objectScaleToSlider(relicSize) * 100),
							`${relicSize.toFixed(1)}x`,
							(percent) => onRelicSizeChange(objectSliderToScale(percent / 100))
						)}
						{@render sizeRow(
							m.pal_height(),
							palHeight,
							`${Math.round(palHeight / 100)}m`,
							onPalHeightChange,
							0,
							5000
						)}
					{/if}
					{@render sizeRow(
						m.map_opacity(),
						Math.round(mapOpacity * 100),
						`${Math.round(mapOpacity * 100)}%`,
						(percent) => onMapOpacityChange(percent / 100),
						MAP_OPACITY_MIN * 100
					)}
				</div>
			{/snippet}
		</Accordion.Item>
	</Accordion>

	{#if structureSections}
		<Button class="bg-primary-500! mt-1.5 flex! w-full!" onclick={handleReset}>
			{m.reset_colors()}
		</Button>
	{/if}
</div>
