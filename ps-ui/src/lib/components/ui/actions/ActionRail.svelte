<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Tooltip } from '$components/ui';
	import { cn } from '$theme';

	import { availableActions, type ActionDescriptor } from './actionDescriptor';

	let {
		actions,
		id,
		class: className
	}: { actions: ActionDescriptor[]; id?: string; class?: string } = $props();

	const rows = $derived(availableActions(actions));
</script>

<nav
	{id}
	class={cn(
		'btn-group preset-outlined-surface-200-800 flex-col items-center self-start rounded-sm',
		className
	)}
>
	{#each rows as action (action.id)}
		<Tooltip label={action.label}>
			<Button variant="ghost" size="icon" aria-label={action.label} onclick={action.run}>
				<Icon icon={action.icon} class="size-6" />
			</Button>
		</Tooltip>
	{/each}
</nav>
