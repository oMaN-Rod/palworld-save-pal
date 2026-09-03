<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Combobox, Input, Tooltip } from '$components/ui';
	import { languages } from '$types';
	import type { AppSettings, SelectOption } from '$types';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils';
	import { cornerArt, theme, themeOptions } from '$states';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { send, sendAndWait } from '$utils/websocketUtils';
	import { MessageType } from '$types';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';

	// Display-mode switching is a Linux-shell concern: the web/worker build has
	// no shell to switch, and the Windows/macOS desktop shells have no tray or
	// browser mode. Baked at build time, this only prunes the WS query for
	// non-desktop builds; the shell reports support and its current mode at
	// runtime, so the control appears (pre-synced) only where it works.
	const isDesktop = PUBLIC_DESKTOP_MODE === 'true';

	// Combination: desktop | browser
	const modeOptions: SelectOption[] = [
		{ value: 'desktop', label: 'Desktop Mode' },
		{ value: 'browser', label: 'System Tray / Browser' }
	];

	// The shell relaunches on a real switch (webview ↔ headless cannot
	// hot-swap), so once fired the current session is leaving anyway — lock the
	// control to avoid a second change racing the relaunch. Re-picking the
	// shell's current mode is a no-op, not a switch.
	let switching = $state(false);
	let displayMode = $state('desktop');
	// The mode the shell reports; null when not chosen yet (first run) or
	// unreadable.
	let shellMode = $state<string | null>(null);
	let modeSupported = $state(false);

	onMount(async () => {
		if (!isDesktop) return;
		try {
			const info = await sendAndWait<{ supported: boolean; mode: string | null }>(
				MessageType.GET_DISPLAY_MODE
			);
			modeSupported = info.supported;
			shellMode = info.mode;
			displayMode = info.mode ?? 'desktop';
		} catch {
			// No mode-switching shell (or the query failed): keep it hidden.
			modeSupported = false;
		}
	});

	function switchMode(mode: string) {
		if (switching || mode === shellMode) return;
		switching = true;
		shellMode = mode;
		send(MessageType.SET_MODE, { mode });
	}

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
			<Combobox options={themeOptions} bind:value={theme.current} label={m.theme()} />
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

		{#if modeSupported}
			<div class="mt-2 border-surface-500/40 border-t pt-2">
				<Combobox
					options={modeOptions}
					bind:value={displayMode}
					label="Display mode"
					onChange={(value) => switchMode(String(value))}
				/>
				<p class="text-muted mt-1 text-xs">
					Switches between the app window and a quiet background tray that opens the editor in
					your browser. The app restarts to apply it.
				</p>
			</div>
		{/if}

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
