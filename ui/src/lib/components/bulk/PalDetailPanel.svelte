<script lang="ts">
	import { Loading } from '$components/ui';
	import { getAppState } from '$states';
	import { goto } from '$app/navigation';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { X } from 'lucide-svelte';
	import { Pencil, PawPrint } from '@lucide/svelte';
	import { PalInfoPopup } from '$components/pal';

	let {
		expanded = false,
		palId = null,
		onclose
	}: { expanded?: boolean; palId?: string | null; onclose?: () => void } = $props();

	const appState = getAppState();

	const pal = $derived.by(() => {
		if (!palId) return undefined;
		const fromPlayer = appState.bulkDetailPlayer?.pals?.[palId];
		if (fromPlayer) return fromPlayer;
		const bases = appState.bulkDetailGuild?.bases ?? {};
		for (const base of Object.values(bases)) {
			const fromBase = base?.pals?.[palId];
			if (fromBase) return fromBase;
		}
		return undefined;
	});

	const loading = $derived(appState.loadingPlayer || appState.loadingGuild);
	const isPlayerOwned = $derived(!!(palId && appState.bulkDetailPlayer?.pals?.[palId]));

	function editPal() {
		if (!pal) return;
		appState.selectedPal = pal;
		goto('/edit/pal');
	}
</script>

<div
	class="bg-surface-800/80 text-on-surface h-[calc(100vh-84px)] shrink-0 overflow-hidden shadow-lg backdrop-blur-md transition-all duration-300 ease-in-out"
	style:width={expanded ? '480px' : '0px'}
>
	<div class="flex h-full w-120 flex-col overflow-y-auto p-4">
		<div class="mb-3 flex items-center justify-between">
			<div class="flex gap-2">
				<span class="font-semibold">{c.pal}</span>
				{#if isPlayerOwned}
					<div class="flex justify-end">
						<button
							class="hover:text-primary-500 rounded p-1"
							onclick={editPal}
							title={m.edit_entity({ entity: c.pal })}
						>
							<Pencil class="h-4 w-4" />
						</button>
					</div>
				{/if}
			</div>
			<button
				class="hover:text-primary-500 rounded p-1"
				onclick={() => onclose?.()}
				aria-label={m.close_drawer()}
			>
				<X class="h-4 w-4" />
			</button>
		</div>
		{#if loading}
			<div class="flex flex-1 items-center justify-center">
				<Loading
					label={m.loading_entity({ entity: c.pal })}
					loadingComplete={!loading}
					icon={PawPrint}
				/>
			</div>
		{:else if pal}
			<div class="flex flex-col gap-3">
				<PalInfoPopup {pal} />
			</div>
		{:else}
			<div class="flex flex-1 items-center justify-center">
				<p class="text-surface-400 text-sm">
					{m.failed_load_entity({ entity: c.pal })}
				</p>
			</div>
		{/if}
	</div>
</div>
