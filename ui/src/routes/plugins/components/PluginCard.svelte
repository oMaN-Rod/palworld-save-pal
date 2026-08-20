<script lang="ts">
	import { Button, Card, Checkbox, Tooltip } from '$components/ui';
	import { cn } from '$theme';
	import type { PluginCommand, PluginSummary } from '$types';
	import CommandForm from './CommandForm.svelte';

	let {
		plugin,
		disabled = false,
		onToggleEnabled,
		onUninstall,
		onEdit,
		onRun
	}: {
		plugin: PluginSummary;
		disabled?: boolean;
		onToggleEnabled: (enabled: boolean) => void;
		onUninstall: () => void;
		onEdit: () => void;
		onRun: (command: PluginCommand, args: Record<string, unknown>) => void;
	} = $props();

	let expandedCommandId: string | null = $state(null);

	function toggleExpanded(commandId: string) {
		expandedCommandId = expandedCommandId === commandId ? null : commandId;
	}
</script>

<Card class="flex flex-col gap-3">
	<div class="flex items-start justify-between gap-4">
		<div class="min-w-0">
			<div class="flex items-center gap-2">
				<span class="truncate font-bold">{plugin.name}</span>
				<span class="text-surface-400 text-xs">v{plugin.version}</span>
				<span
					class={cn(
						'rounded-full px-2 py-0.5 text-xs',
						plugin.bundled
							? 'bg-secondary-500/25 text-secondary-300'
							: 'bg-primary-500/25 text-primary-300'
					)}
				>
					{plugin.bundled ? 'Bundled' : 'User'}
				</span>
			</div>
			{#if plugin.author}
				<div class="text-surface-400 text-xs">by {plugin.author}</div>
			{/if}
			{#if plugin.error}
				<div class="text-error-500 text-xs">{plugin.error}</div>
			{/if}
		</div>

		<div class="flex shrink-0 items-center gap-2">
			<Checkbox
				label="Enabled"
				checked={plugin.enabled}
				onchange={(event) => onToggleEnabled((event.target as HTMLInputElement).checked)}
			/>
			<Button variant="ghost" size="sm" onclick={onEdit}>Edit</Button>
			{#if !plugin.bundled}
				<Button variant="ghost" size="sm" onclick={onUninstall}>Uninstall</Button>
			{/if}
		</div>
	</div>

	{#if plugin.commands.length > 0}
		<div class="flex flex-col gap-2">
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
</Card>
