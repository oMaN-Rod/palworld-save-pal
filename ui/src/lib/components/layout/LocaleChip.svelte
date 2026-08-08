<script lang="ts">
	import { Popover } from '$components/ui';
	import { applyLocale, getAppState } from '$states';
	import { languages, type SupportedLanguage } from '$types';
	import Globe from '@lucide/svelte/icons/globe';

	const appState = getAppState();
	const entries = Object.entries(languages) as [SupportedLanguage, string][];
	const activeCode = $derived(appState.settings.language ?? 'en');
</script>

<Popover position="bottom-end">
	{#snippet children()}
		<button class="public-chip" type="button">
			<Globe class="h-3.5 w-3.5" />
			<span class="uppercase">{activeCode}</span>
		</button>
	{/snippet}
	{#snippet content({ close }: { close: () => void })}
		<div class="flex max-h-72 flex-col gap-0.5 overflow-y-auto">
			{#each entries as [code, label] (code)}
				<button
					type="button"
					class="public-chip-option"
					class:is-active={activeCode === code}
					onclick={() => {
						applyLocale(code);
						close();
					}}
				>
					{label}
				</button>
			{/each}
		</div>
	{/snippet}
</Popover>
