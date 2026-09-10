<script lang="ts">
	import { Button, Tooltip } from '$components/ui';
	import type { PluginCommand, PluginSummary } from '$types';
	import CommandForm from './CommandForm.svelte';

	let {
		plugin,
		disabled = false,
		onRun
	}: {
		plugin: PluginSummary;
		disabled?: boolean;
		onRun: (command: PluginCommand, args: Record<string, unknown>) => void;
	} = $props();

	let expandedCommandId: string | null = $state(null);

	function toggleExpanded(commandId: string) {
		expandedCommandId = expandedCommandId === commandId ? null : commandId;
	}
</script>

{#if plugin.commands.length > 0}
	<div class="flex flex-col gap-2 max-h-158 2xl:max-h-216 overflow-y-auto">
		{#each plugin.commands as command (command.id)}
			<div class="border-surface-700 rounded-sm border p-2">
				<div class="flex items-center justify-between gap-2">
					<div class="min-w-0">
						<span class="font-medium">{command.title}</span>
						{#if command.destructive}
							<Tooltip label="Destructive -- always runs a dry-run preview first">
								<span class="text-warning-500 ml-1 text-xs">destructive</span>
							</Tooltip>
						{/if}
						{#if command.description}
							<p class="text-surface-400 text-xs">{command.description}</p>
						{/if}
					</div>
					<Button
						variant={expandedCommandId === command.id ? 'secondary' : 'ghost'}
						size="sm"
						disabled={!plugin.enabled || disabled}
						onclick={() => toggleExpanded(command.id)}
					>
						{expandedCommandId === command.id ? 'Close' : 'Run'}
					</Button>
				</div>
				{#if expandedCommandId === command.id}
					<div class="mt-2">
						{#key command.id}
							<CommandForm {command} {disabled} onRun={(args) => onRun(command, args)} />
						{/key}
					</div>
				{/if}
			</div>
		{/each}
	</div>
{:else}
	<p class="text-surface-400 text-sm">This plugin declares no commands.</p>
{/if}
