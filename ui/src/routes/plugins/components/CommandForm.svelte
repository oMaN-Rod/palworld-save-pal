<script lang="ts">
	import { Button, Checkbox, Input, Select } from '$components/ui';
	import type { PluginCommand } from '$types';

	let {
		command,
		disabled = false,
		onRun
	}: {
		command: PluginCommand;
		disabled?: boolean;
		onRun: (args: Record<string, unknown>) => void;
	} = $props();

	function defaultValues(cmd: PluginCommand): Record<string, unknown> {
		const out: Record<string, unknown> = {};
		for (const param of cmd.params) {
			if (param.default !== null && param.default !== undefined) {
				out[param.id] = param.default;
				continue;
			}
			switch (param.type) {
				case 'int':
				case 'float':
					out[param.id] = param.min ?? 0;
					break;
				case 'string':
					out[param.id] = '';
					break;
				case 'bool':
					out[param.id] = false;
					break;
				case 'enum':
					out[param.id] = param.options[0] ?? '';
					break;
			}
		}
		return out;
	}

	let values: Record<string, unknown> = $state(defaultValues(command));

	function submit() {
		onRun({ ...values });
	}
</script>

<div class="flex flex-col gap-2">
	{#each command.params as param (param.id)}
		<div>
			{#if param.type === 'int' || param.type === 'float'}
				<Input
					type="number"
					label={param.label}
					step={param.type === 'int' ? 1 : undefined}
					min={param.min ?? undefined}
					max={param.max ?? undefined}
					bind:value={values[param.id] as number}
					{disabled}
				/>
			{:else if param.type === 'string'}
				<Input type="text" label={param.label} bind:value={values[param.id] as string} {disabled} />
			{:else if param.type === 'bool'}
				<Checkbox label={param.label} bind:checked={values[param.id] as boolean} />
			{:else if param.type === 'enum'}
				<Select
					label={param.label}
					options={param.options.map((option) => ({ value: option, label: option }))}
					value={values[param.id] as string}
					onChange={(value) => (values[param.id] = value)}
					{disabled}
				/>
			{/if}
			{#if param.description}
				<p class="text-surface-400 text-xs">{param.description}</p>
			{/if}
		</div>
	{/each}

	<Button size="sm" onclick={submit} {disabled}>
		{command.destructive ? 'Preview' : 'Run'}
	</Button>
</div>
