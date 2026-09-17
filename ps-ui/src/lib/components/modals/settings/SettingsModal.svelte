<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Combobox, Input, Tooltip } from '$components/ui';
	import { languages } from '$types';
	import type { AppSettings, SelectOption } from '$types';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils';
	import { cornerArt, ROSE_RED, rwbySkin, rwbyUnlocked, theme, themeOptions } from '$states';
	import type { ThemeName } from '$states';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import NetworkPanel from '../../settings/NetworkPanel.svelte';

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

	const RWBY_OPTION = 'rwby';
	const themeSelectOptions = $derived(
		rwbyUnlocked.current ? [...themeOptions, { value: RWBY_OPTION, label: 'RWBY' }] : themeOptions
	);
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

	type TabId = 'general' | 'appearance' | 'network';
	let activeTab = $state<TabId>('general');

	// The desktop app is always localhost-only, so its network settings live
	// nowhere; the tab only exists in server/webapp builds.
	const networkTabVisible = PUBLIC_DESKTOP_MODE !== 'true';

	let modalContainer: HTMLDivElement;

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="flex max-h-[85vh] min-h-0 w-[min(760px,calc(100vw-2rem))] flex-col overflow-hidden">
		<div class="border-b border-surface-800 px-4 pb-0 pt-3">
			<h3 class="h3">{title}</h3>
			<div class="mt-2 flex gap-1 overflow-x-auto" role="tablist">
				<button
					type="button"
					role="tab"
					aria-selected={activeTab === 'general'}
					class="flex shrink-0 items-center gap-2 rounded-md px-3 py-1.5 text-sm transition-colors {activeTab === 'general'
						? 'bg-primary-500/15 text-primary-400'
						: 'text-surface-300 hover:bg-surface-800 hover:text-surface-50'}"
					onclick={() => (activeTab = 'general')}
				>
					<Icon icon="tabler:adjustments-horizontal" size={16} />
					{m.settings_tab_general()}
				</button>
				<button
					type="button"
					role="tab"
					aria-selected={activeTab === 'appearance'}
					class="flex shrink-0 items-center gap-2 rounded-md px-3 py-1.5 text-sm transition-colors {activeTab === 'appearance'
						? 'bg-primary-500/15 text-primary-400'
						: 'text-surface-300 hover:bg-surface-800 hover:text-surface-50'}"
					onclick={() => (activeTab = 'appearance')}
				>
					<Icon icon="tabler:palette" size={16} />
					{m.settings_tab_appearance()}
				</button>
				{#if networkTabVisible}
					<button
						type="button"
						role="tab"
						aria-selected={activeTab === 'network'}
						class="flex shrink-0 items-center gap-2 rounded-md px-3 py-1.5 text-sm transition-colors {activeTab === 'network'
							? 'bg-primary-500/15 text-primary-400'
							: 'text-surface-300 hover:bg-surface-800 hover:text-surface-50'}"
						onclick={() => (activeTab = 'network')}
					>
						<Icon icon="tabler:network" size={16} />
						{m.nav_network()}
					</button>
				{/if}
			</div>
		</div>

		<div class="min-h-0 flex-1 overflow-y-auto p-4">
				{#if activeTab === 'general'}
					<div class="flex flex-col gap-3">
						<Combobox options={languageOptions} bind:value={settings.language} label={m.language()} />
						<Input bind:value={settings.clone_prefix} label={m.clone_prefix()} />
						<Input bind:value={settings.new_pal_prefix} label={m.new_pal_prefix()} />
						<div class="flex items-center gap-2">
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
						<div class="flex items-center gap-2">
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
				{:else if activeTab === 'appearance'}
					<div class="flex flex-col gap-3">
						<Combobox
							options={themeSelectOptions}
							bind:value={selectedTheme}
							onChange={handleThemeChange}
							label={m.theme()}
						>
							{#snippet selectOption(option)}
								{#if option.value === RWBY_OPTION}
									<span class="flex items-center gap-1.5">
										<Icon icon="local:rwby-rose" size={16} color={ROSE_RED} />
										{option.label}
									</span>
								{:else}
									{option.label}
								{/if}
							{/snippet}
						</Combobox>
						<!-- Purely visual, so this one is local UI state (like the theme picker
						     above), applied immediately and persisted to localStorage — not part
						     of the backend-persisted AppSettings payload. -->
						<div class="flex items-center gap-2">
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
				{:else}
					<NetworkPanel />
				{/if}
			</div>

			{#if activeTab !== 'network'}
			<div class="flex justify-end gap-2 border-t border-surface-800 p-3">
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
		{/if}
	</Card>
</div>