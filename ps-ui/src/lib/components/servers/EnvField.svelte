<script lang="ts">
	import { Input } from '$components/ui';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import type { EnvKey } from './envGroups';
	import { isTruthy } from './envGroups';
	import type { CheckedChangeDetails } from '@zag-js/switch';

	let {
		envKey,
		value,
		onchange
	} = $props<{
		envKey: EnvKey;
		value: string;
		onchange: (key: string, value: string) => void;
	}>();

	const isBool = $derived(envKey.type === 'bool');
	const checked = $derived(isTruthy(value));
</script>

{#if isBool}
	<div class="flex items-center justify-between rounded-sm px-1 py-2">
		<span class="text-surface-300 text-sm">{envKey.label}</span>
		<Switch
			checked={checked}
			onCheckedChange={(e: CheckedChangeDetails) => onchange(envKey.key, e.checked ? 'true' : 'false')}
		/>
	</div>
{:else}
	<Input
		label={envKey.label}
		{value}
		onValueChange={(v) => onchange(envKey.key, String(v))}
	/>
{/if}
