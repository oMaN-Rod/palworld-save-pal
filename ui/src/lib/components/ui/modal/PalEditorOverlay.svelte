<script lang="ts">
	import { fade } from 'svelte/transition';
	import { getAppState, getModalState, getPalEditorState } from '$states';
	import { Loading } from '$components/ui';
	import { PalEditModal } from '$components/modals';
	import { onMount, onDestroy } from 'svelte';
	import { PawPrint, X } from '@lucide/svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	const palEditor = getPalEditorState();
	const modal = getModalState();
	const appState = getAppState();

	function handleKeydown(event: KeyboardEvent) {
		if (!palEditor.isOpen) return;
		// A sub-modal is stacked on top and owns Escape; let it handle it.
		if (modal.isOpen) return;
		if (event.key === 'Escape') {
			event.preventDefault();
			palEditor.close();
		}
	}

	function handleOutsideClick(event: MouseEvent) {
		if (event.target === event.currentTarget) palEditor.close();
	}

	onMount(() => window.addEventListener('keydown', handleKeydown));
	onDestroy(() => window.removeEventListener('keydown', handleKeydown));
</script>

{#if palEditor.isOpen}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="pal-editor-overlay fixed inset-0 flex items-center justify-center bg-black/60 backdrop-blur-sm"
		transition:fade={{ duration: 200 }}
		onclick={handleOutsideClick}
		onkeydown={handleKeydown}
		role="dialog"
		aria-modal="true"
		tabindex="-1"
	>
		<div class="relative">
			<div class="bg-surface-950 h-[90vh] w-[90vw] overflow-hidden rounded-sm">
				{#if palEditor.loading}
					<div class="flex h-full items-center justify-center">
						<Loading
							label={m.loading_entity({ entity: c.pal })}
							loadingComplete={false}
							icon={PawPrint}
						/>
					</div>
				{:else if appState.selectedPal}
					<PalEditModal />
				{/if}
			</div>
			<button
				type="button"
				class="bg-surface-950 text-surface-200 border-surface-700 hover:bg-surface-800 hover:text-surface-50 absolute top-0 left-full z-20 ml-2 flex size-11 items-center justify-center rounded-full border-2 shadow-lg transition-colors"
				aria-label={m.close()}
				onclick={() => palEditor.close()}
			>
				<X size={24} />
			</button>
		</div>
	</div>
{/if}

<style>
	.pal-editor-overlay {
		z-index: 40000;
	}
</style>
