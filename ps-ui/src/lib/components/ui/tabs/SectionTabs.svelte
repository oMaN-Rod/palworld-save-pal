<script lang="ts" module>
	export type SectionTab = { id: string; label: string };

	export function tabId(prefix: string, id: string): string {
		return `${prefix}-${id}-tab`;
	}

	export function panelId(prefix: string, id: string): string {
		return `${prefix}-${id}-panel`;
	}
</script>

<script lang="ts">
	let {
		tabs,
		active = $bindable(),
		label,
		idPrefix
	}: { tabs: SectionTab[]; active: string; label: string; idPrefix: string } = $props();

	let buttons: (HTMLButtonElement | null)[] = $state([]);

	function activate(index: number) {
		const tab = tabs[index];
		if (!tab) return;
		active = tab.id;
		buttons[index]?.focus();
	}

	function onKeydown(event: KeyboardEvent) {
		const index = tabs.findIndex((tab) => tab.id === active);
		if (index === -1) return;

		switch (event.key) {
			case 'ArrowRight':
				event.preventDefault();
				activate((index + 1) % tabs.length);
				break;
			case 'ArrowLeft':
				event.preventDefault();
				activate((index - 1 + tabs.length) % tabs.length);
				break;
			case 'Home':
				event.preventDefault();
				activate(0);
				break;
			case 'End':
				event.preventDefault();
				activate(tabs.length - 1);
				break;
		}
	}
</script>

<!-- `flex-1` with `min-w-fit`: a few tabs share the width evenly as before, and
     a strip too long for the screen scrolls rather than squeezing its labels
     into unreadable columns. -->
<div
	class="border-surface-700/40 bg-surface-900/40 flex shrink-0 overflow-x-auto border-b"
	role="tablist"
	aria-label={label}
>
	{#each tabs as tab, index (tab.id)}
		<button
			bind:this={buttons[index]}
			id={tabId(idPrefix, tab.id)}
			type="button"
			role="tab"
			aria-controls={panelId(idPrefix, tab.id)}
			aria-selected={tab.id === active}
			tabindex={tab.id === active ? 0 : -1}
			class="h-11 min-w-fit flex-1 px-3 text-sm whitespace-nowrap transition-colors"
			class:font-bold={tab.id === active}
			class:text-primary-300={tab.id === active}
			class:border-primary-400={tab.id === active}
			class:border-b-2={tab.id === active}
			class:text-surface-300={tab.id !== active}
			onclick={() => (active = tab.id)}
			onkeydown={onKeydown}
		>
			{tab.label}
		</button>
	{/each}
</div>
