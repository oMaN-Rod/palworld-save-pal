<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Combobox, Input, Tooltip } from '$components/ui';
	import { languages } from '$types';
	import type { AppSettings, SelectOption } from '$types';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils';
	import { cornerArt, rwbySkin, rwbyUnlocked, theme, themeOptions } from '$states';
	import type { ThemeName } from '$states';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		title = '',
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

	// The RWBY skin is not a real palette — it rides on top of whatever theme
	// is active — so it shows up as a pseudo entry only for those who found the
	// Signal easter egg, and selecting it just flips the skin instead of
	// touching the persisted theme underneath.
	const RWBY_OPTION = 'rwby';
	const themeSelectOptions = $derived(
		rwbyUnlocked.current ? [...themeOptions, { value: RWBY_OPTION, label: 'RWBY' }] : themeOptions
	);
	// Seeded synchronously — the Combobox resolves its display label from the
	// value at mount — then kept in step with toggles made outside the modal.
	let selectedTheme = $state<string>(rwbySkin.current ? RWBY_OPTION : theme.current);
	$effect(() => {
		selectedTheme = rwbySkin.current ? RWBY_OPTION : theme.current;
	});

	function handleThemeChange(value: string | number): void {
		if (value === RWBY_OPTION) {
			rwbySkin.current = true;
		} else {
			rwbySkin.current = false;
			theme.current = value as ThemeName;
		}
	}

	let modalContainer: HTMLDivElement;

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/3)]">
		<h3 class="h3">{title}</h3>

		<div class="mt-2 flex flex-col space-y-2">
			<Combobox options={languageOptions} bind:value={settings.language} label={m.language()} />
			<Combobox
				options={themeSelectOptions}
				bind:value={selectedTheme}
				onChange={handleThemeChange}
				label={m.theme()}
			>
				{#snippet selectOption(option)}
					{#if option.value === RWBY_OPTION}
						<span class="flex items-center gap-1.5">
							<img src="/rwby-rose.webp" alt="" width="16" height="16" class="h-4 w-4" />
							{option.label}
						</span>
					{:else}
						{option.label}
					{/if}
				{/snippet}
			</Combobox>
			<Input bind:value={settings.clone_prefix} label={m.clone_prefix()} />
			<Input bind:value={settings.new_pal_prefix} label={m.new_pal_prefix()} />
			<div class="flex space-x-2">
				<Switch
					checked={settings.debug_mode}
					onCheckedChange={(mode: CheckedChangeDetails) => {
						settings.debug_mode = mode.checked;
					}}
					name="debug_mode"
					label={m.debug_mode()}
				/>
				<span>{m.debug_mode()}</span>
			</div>
			<div class="flex space-x-2">
				<Switch
					checked={settings.cheat_mode}
					onCheckedChange={(mode: CheckedChangeDetails) => {
						settings.cheat_mode = mode.checked;
					}}
					name="cheat_mode"
					label={m.cheat_mode()}
				/>
				<span>{m.cheat_mode()}</span>
			</div>
		</div>

		<div class="mt-2 flex flex-col space-y-2">
			<!-- Purely visual, so this one is local UI state (like the theme picker
			     above), applied immediately and persisted to localStorage — not part
			     of the backend-persisted AppSettings payload. -->
			<div class="flex space-x-2">
				<Switch
					checked={cornerArt.current}
					onCheckedChange={(mode: CheckedChangeDetails) => {
						cornerArt.current = mode.checked;
					}}
					name="corner_art"
					label={m.show_corner_art()}
				/>
				<span>{m.show_corner_art()}</span>
			</div>
		</div>

		<div class="mt-2 flex justify-end space-x-2">
			<Tooltip position="bottom" label={c.save}>
				<Button variant="ghost" size="icon" onclick={() => closeModal(settings)} data-modal-primary>
					<Icon icon="tabler:device-floppy" />
				</Button>
			</Tooltip>

			<Tooltip position="bottom" label={m.cancel()}>
				<Button variant="ghost" size="icon" onclick={() => closeModal(null)}>
					<Icon icon="tabler:x" />
				</Button>
			</Tooltip>
		</div>
	</Card>
</div>
