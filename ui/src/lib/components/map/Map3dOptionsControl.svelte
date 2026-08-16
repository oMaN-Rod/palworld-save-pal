<script lang="ts">
	import type { IControl } from 'maplibre-gl';
	import { untrack } from 'svelte';
	import { getMapContext, type ControlPosition } from '$components/maplibre';
	import { Accordion, Slider, Switch } from '@skeletonlabs/skeleton-svelte';
	import Eye from '@lucide/svelte/icons/eye';
	import EyeClosed from '@lucide/svelte/icons/eye-closed';
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
	import { Button } from '$components/ui';
	import type { ValueChangeDetails } from '@zag-js/slider';
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
		/** Whether the host has bases to draw. Off leaves the structure sections out
		 *  entirely. */
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

	// lucide `settings-2`.
	const OPTIONS_ICON =
		"url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='29' height='29' viewBox='-2.5 -2.5 29 29' fill='none' stroke='%23333' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M20 7h-9'/%3E%3Cpath d='M14 17H5'/%3E%3Ccircle cx='17' cy='17' r='3'/%3E%3Ccircle cx='7' cy='7' r='3'/%3E%3C/svg%3E\")";

	// Every structure section needs both 3D and a host with bases; without the
	// latter they would render against nothing.
	const structureSections = $derived(show3d && showStructureControls);
	const detailedSections = $derived(detailed && showStructureControls);

	const colors = $derived(structureColors());
	const tints = $derived(materialTints());
	const blend = $derived(materialBlend());
	const opacities = $derived(materialOpacities());

	// A base-owning host opens to structures; without bases that section never
	// renders, leaving sizes as the only one to start on.
	let accordionValue = $state<string[]>(
		untrack(() => (showStructureControls ? ['structures'] : ['sizes']))
	);

	// Tracks the drag live so the readout doesn't lag the debounced store write.
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

	// debounce() keeps one timer and fires with the last args, so a shared setter
	// would drop an update when two swatches move inside one window. Pending
	// values accumulate per key and flush together.
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

		// Mirrors Toggle3dControl's markup, plus a panel div appended as a second
		// child so it can open beneath the button.
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
		<Switch {checked} onCheckedChange={() => onchange()} compact classes="h-6 w-6" />
	</label>
{/snippet}

<div
	bind:this={panel}
	use:stopMapGesturesAction
	class={[
		'absolute top-full right-0 z-10 mt-1 max-h-[min(80vh,620px)] min-w-80 overflow-y-auto rounded bg-(--svlibre-ctrl-bg,#fff) p-2 text-(--svlibre-ctrl-color,#333) shadow-(--svlibre-ctrl-shadow,0_0_0_2px_rgba(0,0,0,0.1))',
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
									{#snippet inactiveChild()}<EyeClosed class="h-4 w-4" />{/snippet}
									{#snippet activeChild()}<Eye class="h-4 w-4" />{/snippet}
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
									height="h-1"
									thumbSize="size-3"
									value={[opacityPercents[material]]}
									onValueChange={(e: ValueChangeDetails) =>
										queueMaterialOpacity(material, Number(e.value[0]))}
								/>
								<span class="w-[34px] shrink-0 text-right tabular-nums"
									>{opacityPercents[material]}%</span
								>
							</div>
						{/each}
					</div>
					<label class="mt-1.5 flex items-center gap-1.5 text-xs whitespace-nowrap">
						<span>{m.material_blend()}</span>
						<Slider
							height="h-1"
							thumbSize="size-3"
							value={[blendPercent]}
							onValueChange={(e: ValueChangeDetails) => {
								blendPercent = Number(e.value[0]);
								flushBlend(blendPercent / 100);
							}}
						/>
						<span class="w-8.5 text-right tabular-nums">{blendPercent}%</span>
					</label>
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
						<span>{m.pal_size()}</span>
						<Slider
							height="h-1"
							thumbSize="size-3"
							value={[Math.round(palScaleToSlider(palSize) * 100)]}
							onValueChange={(e: ValueChangeDetails) =>
								onPalSizeChange(palSliderToScale(Number(e.value[0]) / 100))}
						/>
						<span class="w-9.5 shrink-0 text-right tabular-nums">{palSize.toFixed(1)}x</span>
						<span>{m.fast_travel_size()}</span>
						<Slider
							height="h-1"
							thumbSize="size-3"
							value={[Math.round(objectScaleToSlider(fastTravelSize) * 100)]}
							onValueChange={(e: ValueChangeDetails) =>
								onFastTravelSizeChange(objectSliderToScale(Number(e.value[0]) / 100))}
						/>
						<span class="w-9.5 shrink-0 text-right tabular-nums">{fastTravelSize.toFixed(1)}x</span>
						<span>{m.watchtower_size()}</span>
						<Slider
							height="h-1"
							thumbSize="size-3"
							value={[Math.round(objectScaleToSlider(watchtowerSize) * 100)]}
							onValueChange={(e: ValueChangeDetails) =>
								onWatchtowerSizeChange(objectSliderToScale(Number(e.value[0]) / 100))}
						/>
						<span class="w-9.5 shrink-0 text-right tabular-nums">{watchtowerSize.toFixed(1)}x</span>
						<span>{m.relic_size()}</span>
						<Slider
							height="h-1"
							thumbSize="size-3"
							value={[Math.round(objectScaleToSlider(relicSize) * 100)]}
							onValueChange={(e: ValueChangeDetails) =>
								onRelicSizeChange(objectSliderToScale(Number(e.value[0]) / 100))}
						/>
						<span class="w-9.5 shrink-0 text-right tabular-nums">{relicSize.toFixed(1)}x</span>
						<span>{m.pal_height()}</span>
						<Slider
							height="h-1"
							thumbSize="size-3"
							value={[palHeight]}
							max={5000}
							onValueChange={(e: ValueChangeDetails) => onPalHeightChange(Number(e.value[0]))}
						/>
						<span class="w-9.5 shrink-0 text-right tabular-nums"
							>{Math.round(palHeight / 100)}m</span
						>
					{/if}
					<span>{m.map_opacity()}</span>
					<Slider
						height="h-1"
						thumbSize="size-3"
						min={MAP_OPACITY_MIN * 100}
						value={[Math.round(mapOpacity * 100)]}
						onValueChange={(e: ValueChangeDetails) => onMapOpacityChange(Number(e.value[0]) / 100)}
					/>
					<span class="w-9.5 shrink-0 text-right tabular-nums">{Math.round(mapOpacity * 100)}%</span
					>
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
