<script lang="ts">
	import { Button, Card } from '$components/ui';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';

	let { bases, closeModal } = $props<{
		bases: { id: string; name: string; guildName: string }[];
		closeModal: (value: { id: string; name: string } | null) => void;
	}>();

	let modalContainer: HTMLDivElement;

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="max-w-lg min-w-[400px]">
		<h3 class="h3">Choose a base to capture</h3>

		{#if bases.length === 0}
			<p class="text-surface-400 mt-2 text-sm">No bases in the loaded save.</p>
		{:else}
			<div class="mt-2 flex flex-col gap-1">
				{#each bases as base}
					<Button
						variant="ghost"
						class="justify-between"
						onclick={() => closeModal({ id: base.id, name: base.name })}
					>
						<span class="truncate">{base.name || base.id}</span>
						<span class="text-surface-400 text-sm">{base.guildName}</span>
					</Button>
				{/each}
			</div>
		{/if}

		<div class="mt-4 flex justify-end">
			<Button variant="neutral" onclick={() => closeModal(null)}>Cancel</Button>
		</div>
	</Card>
</div>
