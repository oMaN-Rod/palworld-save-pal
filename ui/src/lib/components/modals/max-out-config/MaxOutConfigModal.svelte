<script lang="ts">
	import { Button, Card, Tooltip } from '$components/ui';
	import { maxOutNameDescriptionMap, type MaxOutConfig } from '$types';
	import { focusModal } from '$utils';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import { Check, X } from 'lucide-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';
	import { onMount } from 'svelte';

	let { config, closeModal } = $props<{
		config: MaxOutConfig;
		closeModal: (value: any) => void;
	}>();

	let modalContainer: HTMLDivElement;

	function handleClose(value: any) {
		if (value) {
			closeModal(config);
		} else {
			closeModal(null);
		}
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/3)]">
		<h3 class="h3">Max Out Config</h3>

		<div class="mt-2 flex flex-col space-y-4">
			<div class="grid max-h-[60vh] grid-cols-3 gap-2 overflow-y-auto p-2">
				{#each Object.entries(config) as [property, _]}
					<div class="flex space-x-2">
						<Tooltip position="right" baseClass="flex items-center space-x-2">
							<Switch
								name={maxOutNameDescriptionMap[property]?.label ?? property}
								checked={config[property as keyof MaxOutConfig]}
								onCheckedChange={(mode: CheckedChangeDetails) => {
									config[property as keyof MaxOutConfig] = mode.checked;
								}}
							/>
							<span>{maxOutNameDescriptionMap[property]?.label ?? property}</span>
							{#snippet popup()}
								<span>{maxOutNameDescriptionMap[property]?.description ?? ''}</span>
							{/snippet}
						</Tooltip>
					</div>
				{/each}
			</div>

			<div class="mt-2 flex justify-end space-x-2">
				<Tooltip position="bottom">
					{#snippet children()}
						<Button
							variant="ghost"
							size="icon"
							onclick={() => handleClose(true)}
							data-modal-primary
						>
							<Check />
						</Button>
					{/snippet}
					{#snippet popup()}
						<span>Apply</span>
					{/snippet}
				</Tooltip>
				<Tooltip position="bottom">
					{#snippet children()}
						<Button variant="ghost" size="icon" onclick={() => handleClose(false)}>
							<X />
						</Button>
					{/snippet}
					{#snippet popup()}
						<span>Cancel</span>
					{/snippet}
				</Tooltip>
			</div>
		</div>
	</Card>
</div>
