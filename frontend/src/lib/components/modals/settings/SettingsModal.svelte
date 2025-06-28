<script lang="ts">
	import { Card, Combobox, Input, Tooltip } from '$components/ui';
	import { X, Save } from 'lucide-svelte';
	import { languages } from '$types';
	import type { AppSettings, SelectOption } from '$types';
	import { Switch } from '@skeletonlabs/skeleton-svelte';

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
		<div class="flex space-x-2">
			<Switch
				checked={settings.debug_mode}
				onCheckedChange={(mode) => {
					settings.debug_mode = mode.checked;
				}}
				name="debug_mode"
				label="Debug Mode"
			/>
			<span>Debug Mode</span>
		</div>
			<div class="flex space-x-2">
				<Switch
					checked={settings.cheat_mode}
					onCheckedChange={(mode) => {
						settings.cheat_mode = mode.checked;
					}}
					name="cheat_mode"
					label="Cheat Mode"
				/>
				<span>Cheat Mode</span>
			</div>
	</div>

	<div class="mt-2 flex justify-end space-x-2">
		<Tooltip position="bottom" label="Save">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={() => closeModal(settings)}>
				<Save />
			</button>
		</Tooltip>

		<Tooltip position="bottom" label="Cancel">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={() => closeModal(null)}>
				<X />
			</button>
		</Tooltip>
	</div>
</Card>
