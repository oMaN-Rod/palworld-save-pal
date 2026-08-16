<script lang="ts">
	import { Seo } from '$lib/components/seo';
	import { getAppState } from '$states';
	import { Card, Tooltip } from '$components/ui';
	import Saitama from '$lib/assets/img/app/saitama.webp';
	import githubIcon from '$lib/assets/img/app/github.svg';
	import discordIcon from '$lib/assets/img/app/discord.svg';
	import buyMeACoffee from '$lib/assets/img/app/buymeacoffee.png';
	import { send } from '$utils/websocketUtils';
	import { MessageType } from '$types';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';
	import Palette from '@lucide/svelte/icons/palette';
	import GitMerge from '@lucide/svelte/icons/git-merge';
	import Github from '@lucide/svelte/icons/github';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import X from '@lucide/svelte/icons/x';
	import { fade } from 'svelte/transition';

	const appState = getAppState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	const AUTHOR_URL = 'https://github.com/CyrixJD115';

	let hoveringTop = $state(false);
	let easterEgg = $state(false);

	function openLink(event: MouseEvent, url: string) {
		if (isDesktopMode) {
			event.preventDefault();
			send(MessageType.OPEN_URL, url);
		}
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.code === 'Escape') easterEgg = false;
		else if (hoveringTop && event.code === 'KeyC') easterEgg = true;
	}

	function trackHover(node: HTMLElement) {
		function onEnter() {
			hoveringTop = true;
		}
		function onLeave() {
			hoveringTop = false;
		}
		node.addEventListener('mouseenter', onEnter);
		node.addEventListener('mouseleave', onLeave);
		return {
			destroy() {
				node.removeEventListener('mouseenter', onEnter);
				node.removeEventListener('mouseleave', onLeave);
			}
		};
	}

	function tilt(node: HTMLElement) {
		function onEnter() {
			node.style.transition = 'transform 0.08s ease-out';
		}
		function onMove(e: MouseEvent) {
			const rect = node.getBoundingClientRect();
			const x = e.clientX - rect.left;
			const y = e.clientY - rect.top;
			const rx = ((y - rect.height / 2) / rect.height) * -3;
			const ry = ((x - rect.width / 2) / rect.width) * 3;
			node.style.transform = `perspective(800px) rotateX(${rx}deg) rotateY(${ry}deg)`;
		}
		function onLeave() {
			node.style.transition = 'transform 0.3s cubic-bezier(0.23, 1, 0.32, 1)';
			node.style.transform = 'perspective(800px) rotateX(0deg) rotateY(0deg)';
		}
		node.addEventListener('mouseenter', onEnter);
		node.addEventListener('mousemove', onMove);
		node.addEventListener('mouseleave', onLeave);
		return {
			destroy() {
				node.removeEventListener('mouseenter', onEnter);
				node.removeEventListener('mousemove', onMove);
				node.removeEventListener('mouseleave', onLeave);
			}
		};
	}
</script>

<Seo pathname="/about" title={m.about_meta_title()} description={m.about_meta_description()} />

<svelte:window onkeydown={onKeydown} />

<h1 class="sr-only">{m.about_meta_title()}</h1>

{#if easterEgg}
	<div class="fixed inset-0 z-[100] flex items-center justify-center p-4">
		<button
			class="absolute inset-0 cursor-default bg-black/60 backdrop-blur-sm"
			aria-label={m.compat_dismiss()}
			onclick={() => (easterEgg = false)}
			transition:fade={{ duration: 150 }}
		></button>
		<div
			class="border-primary-400/60 bg-surface-900/95 shadow-glow-paldium relative w-full max-w-sm rounded-lg border-2 px-6 py-5 backdrop-blur-md"
			role="dialog"
			aria-modal="true"
			aria-label="CyrixJD115 contributions"
			transition:fade={{ duration: 150 }}
		>
			<!-- close button -->
			<button
				class="text-muted hover:text-surface-50 absolute right-3 top-3 transition-colors"
				onclick={() => (easterEgg = false)}
				aria-label={m.compat_dismiss()}
			>
				<X size={18} />
			</button>

			<!-- header — title links to GitHub -->
			<a
				href={AUTHOR_URL}
				target="_blank"
				rel="noopener noreferrer"
				class="group flex items-center gap-2.5"
				onclick={(event) => openLink(event, AUTHOR_URL)}
			>
				<Sparkles size={20} class="text-primary-400 shrink-0" />
				<h2 class="heading-gradient text-lg font-bold">CyrixJD115</h2>
				<Github size={16} class="text-surface-400 group-hover:text-primary-300 transition-colors" />
			</a>

			<!-- contributions -->
			<div class="mt-4 space-y-2.5">
				<p class="text-muted text-xs font-semibold tracking-wider uppercase">Contributions</p>
				<div class="flex items-center gap-3 rounded-md bg-surface-800/60 px-3 py-2">
					<Palette size={18} class="text-secondary-400 shrink-0" />
					<div>
						<p class="text-surface-50 text-sm font-medium">Theme UI/UX Overhaul</p>
						<p class="text-muted text-xs">Frontier theme, design tokens, palette system</p>
					</div>
				</div>
				<div class="flex items-center gap-3 rounded-md bg-surface-800/60 px-3 py-2">
					<GitMerge size={18} class="text-tertiary-400 shrink-0" />
					<div>
						<p class="text-surface-50 text-sm font-medium">Breeding Calculator</p>
						<p class="text-muted text-xs">Standalone breeding chain solver + dendrogram</p>
					</div>
				</div>
			</div>

			<!-- footer -->
			<p class="text-muted mt-4 text-center text-xs">
				{m.easter_egg_credit()}
			</p>
		</div>
	</div>
{/if}

<div class="animate-fade-in flex h-full w-full items-center justify-center space-x-2 p-2">
	<div class="flex flex-col space-y-2">
		<div use:trackHover use:tilt class="card-tilt">
			<Card>
				<div class="flex space-x-2">
					<img src={staticIcons.pspWhite} alt="Palworld Save Pal" class="mb-2" />
					<span class="font-bold">{appState.version ? `v${appState.version}` : ''}</span>
				</div>
				<hr class="border-surface-500" />
				<div class="mt-2 flex flex-col space-y-2">
					<Tooltip position="left" background="bg-transparent">
						<p>{@html m.about_built_by()}</p>
						{#snippet popup()}
							<img src={Saitama} alt="Saitama" class="inline-block h-48 w-48" />
						{/snippet}
					</Tooltip>
				</div>
			</Card>
		</div>
		<div use:tilt class="card-tilt">
			<Card>
				<div class="flex w-full justify-between gap-2 px-4">
					<a
						href="https://github.com/oMaN-Rod/palworld-save-pal"
						target="_blank"
						rel="noopener noreferrer"
						class="z-10 flex flex-col items-center gap-2 transition-opacity hover:opacity-80"
						onclick={(event) => openLink(event, 'https://github.com/oMaN-Rod/palworld-save-pal')}
					>
						<img src={githubIcon} alt="GitHub" class="h-8 w-8" />
						<span class="align-bottom text-xs">{m.about_link_github()}</span>
					</a>
					<a
						href="https://discord.gg/YWZFPy9G8J"
						target="_blank"
						rel="noopener noreferrer"
						class="z-10 flex flex-col items-center gap-2 transition-opacity hover:opacity-80"
						onclick={(event) => openLink(event, 'https://discord.gg/YWZFPy9G8J')}
					>
						<img src={discordIcon} alt="Discord" class="h-8 w-8" />
						<span class="align-bottom text-xs">{m.about_link_discord()}</span>
					</a>
					<a
						href="https://buymeacoffee.com/i_am_o"
						target="_blank"
						rel="noopener noreferrer"
						class="z-10 flex flex-col items-center gap-2 transition-opacity hover:opacity-80"
						onclick={(event) => openLink(event, 'https://buymeacoffee.com/i_am_o')}
					>
						<img src={buyMeACoffee} alt="Buy me a coffee" class="h-8" />
						<span class="align-bottom text-xs">{m.about_link_support()}</span>
					</a>
				</div>
			</Card>
		</div>
		<div use:tilt class="card-tilt">
			<Card>
				<div class="flex-col space-y-2">
					<h4 class="h4">{m.shortcuts()}</h4>
					<div class="grid grid-cols-1 sm:grid-cols-2">
						<div class="flex items-center">
							<img src={staticIcons.f5Icon} alt="Right Click" class="shortcut-icon" />
							<span class="mx-1">/</span>
							<img src={staticIcons.ctrlIcon} alt="Right Click" class="shortcut-icon" />
							<img src={staticIcons.rIcon} alt="Right Click" class="shortcut-icon" />
						</div>
						<span> {m.refresh()} </span>
						<div class="flex items-center">
							<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
							<img src={staticIcons.plusIcon} alt="Right Click" class="shortcut-icon" />
							<span class="mx-1">/</span>
							<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
							<img src={staticIcons.minusIcon} alt="Right Click" class="shortcut-icon" />
						</div>
						<span>{m.zoom_in_out()}</span>
						<div class="flex items-center">
							<img src={staticIcons.rightClickIcon} alt="Right Click" class="shortcut-icon" />
						</div>
						<span>{m.copy()}</span>
						<div class="flex items-center">
							<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
							<img src={staticIcons.rightClickIcon} alt="Right Click" class="shortcut-icon" />
						</div>
						<span>{m.paste()}</span>
						<div class="flex items-center">
							<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
							<img src={staticIcons.middleClickIcon} alt="Right Click" class="shortcut-icon" />
						</div>
						<span>{m.delete()}</span>
						<div class="flex items-center">
							<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
							<img src={staticIcons.leftClickIcon} alt="Left Click" class="shortcut-icon" />
						</div>
						<span>{m.select()}</span>
					</div>
				</div>
			</Card>
		</div>
	</div>
</div>

<style lang="postcss">
	.shortcut-icon {
		height: 32px;
		width: 32px;
	}
	.card-tilt {
		transition: transform 0.3s cubic-bezier(0.23, 1, 0.32, 1);
		transform-style: preserve-3d;
	}
</style>
