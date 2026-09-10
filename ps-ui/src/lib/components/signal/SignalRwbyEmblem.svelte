<script lang="ts">
	import { fade, scale } from 'svelte/transition';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button } from '$components/ui';
	import { getToastState, ROSE_RED, rwbySkin, rwbyUnlocked } from '$states';
	import * as m from '$i18n/messages';

	const RWBY_CODE = 'rwby';

	let loreOpen = $state(false);
	let hoveringLore = $state(false);
	let codeProgress = 0;

	const toast = getToastState();

	function handleSecretKeydown(event: KeyboardEvent): void {
		if (!loreOpen) return;
		if (event.key === 'Escape') {
			loreOpen = false;
			return;
		}
		if (!hoveringLore || rwbyUnlocked.current) return;
		const target = event.target as HTMLElement | null;
		if (target && /^(input|textarea|select)$/i.test(target.tagName)) return;
		if (event.key.length !== 1) return;
		const key = event.key.toLowerCase();
		if (key === RWBY_CODE[codeProgress]) {
			codeProgress += 1;
			if (codeProgress === RWBY_CODE.length) {
				rwbyUnlocked.current = true;
			}
		} else {
			codeProgress = key === RWBY_CODE[0] ? 1 : 0;
		}
	}

	function toggleRwbySkin(): void {
		rwbySkin.current = !rwbySkin.current;
	}

	function resetRwbySecret(): void {
		rwbyUnlocked.current = false;
		rwbySkin.current = false;
		codeProgress = 0;
		toast.add(m.signal_rwby_reset_toast(), m.signal(), 'info');
	}
</script>

<svelte:window onkeydown={handleSecretKeydown} />

<span class="inline-flex shrink-0 items-center gap-1.5">
	<button
		type="button"
		class="group shrink-0 cursor-help rounded-full transition-transform duration-200 hover:scale-125 hover:rotate-12"
		title={m.signal_rwby_rose_title()}
		aria-label={m.signal_rwby_rose_aria()}
		onclick={() => (loreOpen = true)}
	>
		<Icon
			icon="local:rwby-rose"
			size={20}
			color={ROSE_RED}
			class={rwbySkin.current
				? 'drop-shadow-[0_0_6px_rgb(238_52_80_/_0.8)]'
				: 'opacity-80 group-hover:opacity-100'}
		/>
	</button>
	{#if rwbyUnlocked.current}
		<button
			type="button"
			class="flex items-center gap-1.5 rounded-sm px-2.5 py-2 text-xs font-medium transition-all {rwbySkin.current
				? 'bg-primary-500/15 text-primary-300 border-primary-500/40 border'
				: 'text-surface-300 hover:bg-surface-800 border border-transparent'}"
			title={rwbySkin.current ? m.signal_rwby_toggle_on_title() : m.signal_rwby_toggle_off_title()}
			onclick={toggleRwbySkin}
		>
			<Icon icon="local:rwby-rose" size={14} color={ROSE_RED} />
			{rwbySkin.current ? m.signal_rwby_toggle_on_label() : m.signal_rwby_toggle_off_label()}
		</button>
	{/if}
</span>

{#if loreOpen}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
		transition:fade={{ duration: 150 }}
		role="presentation"
	>
		<button
			type="button"
			class="absolute inset-0 cursor-default"
			aria-label={m.signal_rwby_close_aria()}
			onclick={() => (loreOpen = false)}
		></button>
		<div
			class="card relative z-10 flex max-w-md flex-col items-center gap-3 p-6 text-center"
			transition:scale={{ duration: 150, start: 0.95 }}
			role="dialog"
			aria-modal="true"
			aria-label={m.signal_rwby_lore_dialog_aria()}
			tabindex="-1"
			onmouseenter={() => (hoveringLore = true)}
			onmouseleave={() => {
				hoveringLore = false;
				codeProgress = 0;
			}}
		>
			<Icon
				icon="local:rwby-rose"
				size={72}
				color={ROSE_RED}
				role="img"
				aria-label="A flaming rose emblem"
				class="drop-shadow-[0_0_18px_rgb(238_52_80_/_0.45)]"
			/>
			<h2 class="heading-gradient text-lg font-bold">{m.signal_rwby_lore_heading()}</h2>
			<p class="text-surface-300 text-sm leading-relaxed">
				{@html m.signal_rwby_lore_body()}
			</p>
			{#if rwbyUnlocked.current}
				<div class="mt-1 flex w-full flex-col items-center gap-2">
					<div class="bg-primary-500/10 border-primary-500/30 w-full rounded-sm border p-3">
						<p class="text-primary-200 text-xs font-medium">{m.signal_rwby_lore_unlocked_line()}</p>
					</div>
					<Button
						variant={rwbySkin.current ? 'secondary' : 'primary'}
						size="sm"
						onclick={toggleRwbySkin}
					>
						<Icon icon={rwbySkin.current ? 'tabler:arrow-back-up' : 'tabler:paint'} size={14} />
						{rwbySkin.current ? m.signal_rwby_reskin_off() : m.signal_rwby_reskin_on()}
					</Button>
					<button
						type="button"
						class="text-surface-500 hover:text-surface-300 cursor-pointer text-xs underline-offset-2 transition-colors hover:underline"
						onclick={resetRwbySecret}
					>
						{m.signal_rwby_hide_secret()}
					</button>
				</div>
			{:else}
				<p class="text-surface-500 mt-1 text-xs italic">{m.signal_rwby_lore_hint()}</p>
			{/if}
			<button
				type="button"
				class="text-surface-500 hover:text-surface-300 absolute top-2 right-2 cursor-pointer p-1 transition-colors"
				aria-label={m.signal_rwby_close_aria()}
				onclick={() => (loreOpen = false)}
			>
				<Icon icon="tabler:x" size={16} />
			</button>
		</div>
	</div>
{/if}
