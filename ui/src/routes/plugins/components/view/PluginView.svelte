<script lang="ts">
	import type { PluginCommand } from '$types';
	import type { ViewWidget } from '$lib/plugins/pluginView';
	import type { PluginViewState } from '$lib/plugins/viewState.svelte';
	import ViewSection from './ViewSection.svelte';

	let {
		state,
		commands,
		disabled = false,
		onRun
	}: {
		state: PluginViewState;
		commands: PluginCommand[];
		disabled?: boolean;
		onRun: (widget: ViewWidget) => void;
	} = $props();
</script>

<div class="flex flex-col gap-4">
	{#each state.sections as section, index (index)}
		<ViewSection {section} {state} {commands} {disabled} {onRun} />
	{/each}
</div>
