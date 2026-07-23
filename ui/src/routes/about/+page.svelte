<script lang="ts">
	import { getAppState } from '$states';
		import { Card, Tooltip } from '$components/ui';
		import githubIcon from '$lib/assets/img/app/github.svg';
		import discordIcon from '$lib/assets/img/app/discord.svg';
		import buyMeACoffee from '$lib/assets/img/app/buymeacoffee.png';
		import Saitama from '$lib/assets/img/app/saitama.webp';
		import { send } from '$utils/websocketUtils';
	import { MessageType } from '$types';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';

	const appState = getAppState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	let hoveringTop = $state(false);
	let easterEgg = $state(false);

	function trackHover(node: HTMLElement) {
		node.addEventListener('mouseenter', () => (hoveringTop = true));
		node.addEventListener('mouseleave', () => (hoveringTop = false));
		return {
			destroy() {
				node.removeEventListener('mouseenter', () => (hoveringTop = false));
				node.removeEventListener('mouseleave', () => (hoveringTop = false));
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

<svelte:window onkeydown={(e) => {
	if (hoveringTop && e.code === 'KeyC') easterEgg = true;
}} />

{#if easterEgg}
	<div
		class="fixed inset-0 z-[100] flex items-start justify-end p-4"
		onclick={() => (easterEgg = false)}
	>
		<div
			class="rotate-2 rounded-lg border-2 border-primary-400/60 bg-surface-900/95 px-5 py-4 shadow-glow-paldium backdrop-blur-md animate-fade-in"
			onclick={(e) => e.stopPropagation()}
			role="dialog"
		>
			<p class="heading-gradient text-sm font-bold">🥚 Easter Egg!</p>
			<a
				href="https://github.com/CyrixJD115"
				target="_blank"
				rel="noopener noreferrer"
				class="mt-1 flex items-center gap-2 text-sm font-semibold text-primary-300 underline hover:text-primary-200"
				onclick={(it) => {
					if (isDesktopMode) { it.preventDefault(); send(MessageType.OPEN_URL, 'https://github.com/CyrixJD115'); }
				}}
			>
				CyrixJD115
			</a>
			<p class="mt-0.5 text-[11px] text-muted">Did the GUI overhaul :)</p>
			<button
				class="mt-2 text-[10px] text-muted underline hover:text-surface-50"
				onclick={() => (easterEgg = false)}
			>
				✕ dismiss
			</button>
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
				<div class="flex gap-2 w-full justify-between px-4">
					<a
						href="https://github.com/oMaN-Rod/palworld-save-pal"
						target="_blank"
						rel="noopener noreferrer"
						class="z-10 flex flex-col items-center gap-2 transition-opacity hover:opacity-80"
						onclick={(event) => {
							if (isDesktopMode) { event.preventDefault(); send(MessageType.OPEN_URL, 'https://github.com/oMaN-Rod/palworld-save-pal'); }
						}}
					>
						<img src={githubIcon} alt="GitHub" class="h-8 w-8" />
						<span class="text-xs align-bottom">{m.about_link_github()}</span>
					</a>
					<a
						href="https://discord.gg/YWZFPy9G8J"
						target="_blank"
						rel="noopener noreferrer"
						class="z-10 flex flex-col items-center gap-2 transition-opacity hover:opacity-80"
						onclick={(event) => {
							if (isDesktopMode) { event.preventDefault(); send(MessageType.OPEN_URL, 'https://discord.gg/YWZFPy9G8J'); }
						}}
					>
						<img src={discordIcon} alt="Discord" class="h-8 w-8" />
						<span class="text-xs align-bottom">{m.about_link_discord()}</span>
					</a>
					<a
						href="https://buymeacoffee.com/i_am_o"
						target="_blank"
						rel="noopener noreferrer"
						class="z-10 flex flex-col items-center gap-2 transition-opacity hover:opacity-80"
						onclick={(event) => {
							if (isDesktopMode) { event.preventDefault(); send(MessageType.OPEN_URL, 'https://buymeacoffee.com/i_am_o'); }
						}}
					>
						<img src={buyMeACoffee} alt="Buy me a coffee" class="h-8" />
						<span class="text-xs align-bottom">{m.about_link_support()}</span>
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
