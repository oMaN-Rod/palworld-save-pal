<script lang="ts">
	import { Card, Combobox, Tooltip } from '$components/ui';
	import { X, Save } from 'lucide-svelte';
	import { languages } from '$types';
	import type { SelectOption } from '$types';

	let {
		title = 'Select Language',
		value = $bindable('en'),
		closeModal
	} = $props<{
		title?: string;
		value?: string;
		closeModal: (value: string | null) => void;
	}>();

	const languageOptions: SelectOption[] = Object.entries(languages).map(([code, name]) => ({
		value: code,
		label: name
	}));
</script>

<Card class="bg-surface-500 min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>

	<div class="mt-2">
		<Combobox options={languageOptions} bind:value label="Language" />
	</div>

	<div class="mt-2 flex justify-end space-x-2">
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={() => closeModal(value)}>
				<Save />
			</button>
			{#snippet popup()}
				<span>Save</span>
			{/snippet}
		</Tooltip>

		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={() => closeModal(null)}>
				<X />
			</button>
			{#snippet popup()}
				<span>Cancel</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>
