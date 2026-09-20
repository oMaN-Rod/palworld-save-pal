<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Tooltip } from '$components/ui';

	import { availableActions, type ActionDescriptor } from './actionDescriptor';

	let { actions, id }: { actions: ActionDescriptor[]; id?: string } = $props();

	const rows = $derived(availableActions(actions));
</script>

<nav
	{id}
	class="btn-group preset-outlined-surface-200-800 flex-col items-center self-start rounded-sm"
>
	{#each rows as action (action.id)}
		<Tooltip label={action.label}>
			<Button variant="ghost" size="icon" aria-label={action.label} onclick={action.run}>
				<Icon icon={action.icon} class="size-6" />
			</Button>
		</Tooltip>
	{/each}
</nav>
