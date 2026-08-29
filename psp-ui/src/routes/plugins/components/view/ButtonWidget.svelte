<script lang="ts">
	import { Button, Tooltip } from '$components/ui';
	import type { PluginCommand } from '$types';
	import type { ViewWidget } from '$lib/plugins/pluginView';

	let {
		widget,
		command,
		disabled = false,
		onRun
	}: {
		widget: ViewWidget;
		command: PluginCommand | undefined;
		disabled?: boolean;
		onRun: () => void;
	} = $props();
</script>

<div class="flex items-center gap-2">
	<Button size="sm" {disabled} onclick={onRun}>
		{widget.label ?? command?.title ?? 'Run'}
	</Button>
	{#if command?.destructive}
		<Tooltip label="Destructive -- always runs a dry-run preview first">
			<span class="text-warning-500 text-xs">destructive</span>
		</Tooltip>
	{/if}
</div>
