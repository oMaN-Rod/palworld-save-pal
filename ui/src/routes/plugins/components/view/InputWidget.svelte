<script lang="ts">
	import { Checkbox, Input, Select } from '$components/ui';
	import type { PluginParam } from '$types';
	import type { ViewWidget } from '$lib/plugins/pluginView';
	import type { PluginViewState } from '$lib/plugins/viewState.svelte';

	let {
		widget,
		param,
		state,
		disabled = false
	}: {
		widget: ViewWidget;
		param: PluginParam | undefined;
		state: PluginViewState;
		disabled?: boolean;
	} = $props();

	const label = $derived(widget.label ?? param?.label ?? widget.id ?? '');
	const entity = $derived(widget.entity ? state.optionsFor(widget.entity) : null);
	const entityOptions = $derived([
		{ value: '', label: 'Any' },
		...(entity?.options ?? []).map((option) => ({ value: option.id, label: option.label }))
	]);
	const paramOptions = $derived((param?.options ?? []).map((o) => ({ value: o, label: o })));

	function set(value: unknown) {
		if (widget.id) state.setValue(widget.id, value);
	}

	function toggleMulti(value: string) {
		if (!widget.id) return;
		const current = Array.isArray(state.valueFor(widget.id))
			? (state.valueFor(widget.id) as string[])
			: [];
		set(current.includes(value) ? current.filter((v) => v !== value) : [...current, value]);
	}
</script>

<div>
	{#if widget.type === 'entity_select'}
		<Select
			{label}
			options={entityOptions}
			value={String(widget.id ? (state.valueFor(widget.id) ?? '') : '')}
			onChange={(value) => set(value)}
			disabled={disabled || entityOptions.length <= 1}
		/>
		{#if entity && entity.total > entity.options.length}
			<p class="text-surface-400 text-xs">
				Showing {entity.options.length} of {entity.total}
			</p>
		{:else if entity && entity.total === 0}
			<p class="text-surface-400 text-xs">No save loaded.</p>
		{/if}
	{:else if widget.type === 'number_input'}
		<Input
			type="number"
			{label}
			step={param?.type === 'float' ? undefined : 1}
			min={param?.min ?? undefined}
			max={param?.max ?? undefined}
			value={Number(widget.id ? (state.valueFor(widget.id) ?? 0) : 0)}
			onValueChange={(next) => set(next)}
			{disabled}
		/>
	{:else if widget.type === 'text_input'}
		<Input
			type="text"
			{label}
			value={String(widget.id ? (state.valueFor(widget.id) ?? '') : '')}
			onValueChange={(next) => set(next)}
			{disabled}
		/>
	{:else if widget.type === 'toggle'}
		<Checkbox
			{label}
			{disabled}
			checked={widget.id ? state.valueFor(widget.id) === true : false}
			onchange={(event) => set((event.currentTarget as HTMLInputElement).checked)}
		/>
	{:else if widget.type === 'select'}
		<Select
			{label}
			options={paramOptions}
			value={String(widget.id ? (state.valueFor(widget.id) ?? '') : '')}
			onChange={(value) => set(value)}
			{disabled}
		/>
	{:else if widget.type === 'multiselect'}
		<fieldset class="border-surface-700 rounded-sm border p-2">
			<legend class="text-surface-400 px-1 text-xs">{label}</legend>
			{#each param?.options ?? [] as option (option)}
				<label class="flex items-center gap-2 text-sm">
					<input
						type="checkbox"
						checked={Array.isArray(widget.id ? state.valueFor(widget.id) : null) &&
							(state.valueFor(widget.id ?? '') as string[]).includes(option)}
						onchange={() => toggleMulti(option)}
						{disabled}
					/>
					{option}
				</label>
			{/each}
			{#if (param?.options ?? []).length === 0}
				<p class="text-surface-400 text-sm">Fed by another widget.</p>
			{/if}
		</fieldset>
	{/if}
	{#if param?.description}
		<p class="text-surface-400 text-xs">{param.description}</p>
	{/if}
</div>
