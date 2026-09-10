<script lang="ts">
	import type { PluginCommand } from '$types';
	import type { ViewSection, ViewWidget } from '$lib/plugins/pluginView';
	import type { PluginViewState } from '$lib/plugins/viewState.svelte';
	import ButtonWidget from './ButtonWidget.svelte';
	import InputWidget from './InputWidget.svelte';
	import ListWidget from './ListWidget.svelte';
	import TableWidget from './TableWidget.svelte';
	import TextWidget from './TextWidget.svelte';

	let {
		section,
		state,
		commands,
		disabled = false,
		onRun
	}: {
		section: ViewSection;
		state: PluginViewState;
		commands: PluginCommand[];
		disabled?: boolean;
		onRun: (widget: ViewWidget) => void;
	} = $props();

	const gridClass = $derived(
		section.columns === 3 ? 'sm:grid-cols-3' : section.columns === 2 ? 'sm:grid-cols-2' : ''
	);

	function paramFor(widget: ViewWidget) {
		if (!widget.id) return undefined;
		for (const command of commands) {
			const param = command.params.find((p) => p.id === widget.id);
			if (param) return param;
		}
		return undefined;
	}

	function commandFor(widget: ViewWidget) {
		return commands.find((command) => command.id === widget.command);
	}
</script>

<section class="flex flex-col gap-2">
	{#if section.title}
		<h3 class="text-surface-200 text-sm font-semibold">{section.title}</h3>
	{/if}
	<div class={`grid grid-cols-1 gap-3 items-center ${gridClass}`}>
		{#each section.widgets as widget, index (index)}
			<div class={widget.span === 'full' ? 'col-span-full' : ''}>
				{#if widget.type === 'table'}
					<TableWidget {widget} {state} />
				{:else if widget.type === 'list'}
					<ListWidget {widget} results={state.results} />
				{:else if widget.type === 'text'}
					<TextWidget {widget} results={state.results} />
				{:else if widget.type === 'button'}
					<ButtonWidget {widget} command={commandFor(widget)} {disabled} onRun={() => onRun(widget)} />
				{:else}
					<InputWidget {widget} param={paramFor(widget)} {state} {disabled} />
				{/if}
			</div>
		{/each}
	</div>
</section>
