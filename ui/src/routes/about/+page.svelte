<script lang="ts">
	import { getAppState } from '$states';
	import { Card, Tooltip, TooltipButton } from '$components/ui';
	import { Github } from 'lucide-svelte';
	import Saitama from '$lib/assets/img/app/saitama.webp';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	const appState = getAppState();

	function tilt(node: HTMLElement) {
		function onEnter() {
			node.style.transition = 'transform 0.08s ease-out';
		}
		function onMove(e: MouseEvent) {
			const rect = node.getBoundingClientRect();
			const x = e.clientX - rect.left;
			const y = e.clientY - rect.top;
			const rx = ((y - rect.height / 2) / rect.height) * -8;
			const ry = ((x - rect.width / 2) / rect.width) * 8;
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

<div class="animate-fade-in flex h-full w-full items-center justify-center space-x-2 p-2">
	<div class="flex flex-col space-y-2">
		<div use:tilt class="card-tilt">
			<Card>
				<div class="flex space-x-2">
					<img src={staticIcons.pspWhite} alt="Palworld Save Pal" class="mb-2" />
					<span class="font-bold">{appState.version ? `v${appState.version}` : ''}</span>
				</div>
				<hr class="border-surface-500" />
				<div class="mt-2 flex flex-col space-y-2">
					<Tooltip position="left" background="bg-transparent">
						{@html m.about_built_by()}
						{#snippet popup()}
							<img src={Saitama} alt="Saitama" class="inline-block h-48 w-48" />
						{/snippet}
					</Tooltip>
					<div class="flex items-center space-x-2">
						<span>{m.about_check_out()}</span>
						<a
							href="https://github.com/oMaN-Rod/palworld-save-pal"
							target="_blank"
							rel="noopener noreferrer"
							class="text-primary-400 hover:text-primary-300 z-10 hover:underline"><Github /></a
						>
						<span>{m.about_for_more_info()}</span>
					</div>
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
