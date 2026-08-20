<script lang="ts">
	import { Button, Checkbox, Popover, Select } from '$components/ui';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import type { DraftCommand } from '$lib/plugins/pluginEditor.svelte';

	let {
		commands,
		running,
		bundled = false,
		onRun
	}: {
		commands: DraftCommand[];
		running: boolean;
		bundled?: boolean;
		onRun: (commandId: string, dryRun: boolean) => void;
	} = $props();

	// svelte-ignore state_referenced_locally
	let selected = $state(commands[0]?.id ?? '');
	let dryRun = $state(true);

	$effect(() => {
		if (!commands.some((command) => command.id === selected)) selected = commands[0]?.id ?? '';
	});

	const options = $derived(commands.map((command) => ({ value: command.id, label: command.id })));
	const destructive = $derived(
		commands.find((command) => command.id === selected)?.destructive ?? false
	);
	const effectiveDryRun = $derived(destructive || dryRun);
	const disabled = $derived(bundled || running || selected === '');
</script>

<div class="flex items-stretch">
	<Button
		size="sm"
		{disabled}
		class="rounded-r-none"
		onclick={() => onRun(selected, effectiveDryRun)}
	>
		{running ? 'Running…' : effectiveDryRun ? 'Run dry' : 'Run'}
	</Button>
	<Popover position="bottom-end" popoverClass="w-64" class="flex">
		{#snippet children()}
			<Button size="sm" variant="secondary" class="rounded-l-none px-1" aria-label="Run options">
				<ChevronDown class="h-4 w-4" />
			</Button>
		{/snippet}
		{#snippet content()}
			<div class="flex flex-col gap-2">
				<Select
					{options}
					bind:value={selected}
					onChange={(value) => (selected = String(value))}
					disabled={bundled || commands.length === 0}
				/>
				<Checkbox
					label="Dry run"
					checked={effectiveDryRun}
					disabled={bundled || destructive}
					onchange={(event) => (dryRun = (event.target as HTMLInputElement).checked)}
				/>
				{#if bundled}
					<span class="text-surface-400 text-xs">
						Bundled — read only, so there is no draft to run
					</span>
				{:else if destructive}
					<span class="text-warning-500 text-xs">
						Destructive — always previews with a dry run first
					</span>
				{/if}
			</div>
		{/snippet}
	</Popover>
</div>
