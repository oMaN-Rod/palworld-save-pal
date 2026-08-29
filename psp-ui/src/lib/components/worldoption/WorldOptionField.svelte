<script module lang="ts">
	export type WoValue = boolean | number | string | string[];
</script>

<script lang="ts">
	import { Input } from '$components/ui';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';
	import { cn } from '$theme';
	import type { WoField } from './worldOptionFields';
	import CrossplayPlatformsField from './CrossplayPlatformsField.svelte';
	import DenyTechnologyField from './DenyTechnologyField.svelte';

	let {
		field,
		value,
		isSet,
		technologies = [],
		onchange
	}: {
		field: WoField;
		value: WoValue;
		/** False when the save omits this key: we show the default, badged "not set". */
		isSet: boolean;
		technologies?: string[];
		onchange: (key: string, value: WoValue) => void;
	} = $props();

	function clampNumber(raw: number): number {
		let next = raw;
		if (field.min !== undefined) next = Math.max(field.min, next);
		if (field.max !== undefined) next = Math.min(field.max, next);
		return field.kind === 'int' ? Math.round(next) : next;
	}
</script>

<div class="flex flex-col gap-1">
	{#if field.kind === 'bool'}
		<div class="flex items-center justify-between gap-2">
			<span class="text-sm">
				{field.label}
				{#if !isSet}<span class="text-surface-400 ml-1 text-xs">(not set)</span>{/if}
			</span>
			<Switch
				name={field.key}
				checked={value as boolean}
				onCheckedChange={(e: CheckedChangeDetails) => onchange(field.key, e.checked)}
			/>
		</div>
	{:else if field.kind === 'enum'}
		<label class="text-sm" for={field.key}>
			{field.label}
			{#if !isSet}<span class="text-surface-400 ml-1 text-xs">(not set)</span>{/if}
		</label>
		<select
			id={field.key}
			class={cn('bg-surface-900 rounded-sm px-2 py-1 text-sm')}
			value={value as string}
			onchange={(e) => onchange(field.key, e.currentTarget.value)}
		>
			{#each field.options ?? [] as option (option.value)}
				<option value={option.value}>{option.label}</option>
			{/each}
		</select>
	{:else if field.kind === 'enum_array'}
		<span class="text-sm">
			{field.label}
			{#if !isSet}<span class="text-surface-400 ml-1 text-xs">(not set)</span>{/if}
		</span>
		<CrossplayPlatformsField
			value={value as string[]}
			onchange={(next) => onchange(field.key, next)}
		/>
	{:else if field.kind === 'name_array'}
		<span class="text-sm">
			{field.label}
			{#if !isSet}<span class="text-surface-400 ml-1 text-xs">(not set)</span>{/if}
		</span>
		<DenyTechnologyField
			value={value as string[]}
			{technologies}
			onchange={(next) => onchange(field.key, next)}
		/>
	{:else if field.kind === 'int' || field.kind === 'float'}
		<Input
			label={isSet ? field.label : `${field.label} (not set)`}
			type="number"
			value={value as number}
			min={field.min}
			max={field.max}
			step={field.step}
			onValueChange={(v) => onchange(field.key, clampNumber(Number(v)))}
		/>
	{:else}
		<Input
			label={isSet ? field.label : `${field.label} (not set)`}
			value={value as string}
			onValueChange={(v) => onchange(field.key, String(v))}
		/>
	{/if}
</div>
