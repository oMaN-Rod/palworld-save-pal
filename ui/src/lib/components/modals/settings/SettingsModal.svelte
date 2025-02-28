<script lang="ts">
	import { Card, Combobox, Input, Tooltip } from '$components/ui';
	import { X, Save } from 'lucide-svelte';
	import { languages } from '$types';
	import type { AppSettings, SelectOption } from '$types';

	let {
		title = 'Select Language',
		settings,
		closeModal
	} = $props<{
		title?: string;
		settings?: AppSettings;
		closeModal: (value: AppSettings) => void;
	}>();

	const languageOptions: SelectOption[] = Object.entries(languages).map(([code, name]) => ({
		value: code,
		label: name
	}));
</script>

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>

	<div class="mt-2 flex flex-col space-y-2">
		<Combobox options={languageOptions} bind:value={settings.language} label="Language" />
		<Input bind:value={settings.clone_prefix} label="Clone Prefix" />
		<Input bind:value={settings.new_pal_prefix} label="New Pal Prefix" />
	</div>

	<div class="mt-2 flex justify-end space-x-2">
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={() => closeModal(settings)}>
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
