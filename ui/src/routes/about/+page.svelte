<script lang="ts">
	import { getAppState } from '$states';
	import { Card, Tooltip, TooltipButton } from '$components/ui';
	import { Github } from 'lucide-svelte';
	import Saitama from '$lib/assets/img/app/saitama.webp';
	import { staticIcons } from '$types/icons';

	const appState = getAppState();
</script>

<div class="flex h-full w-full items-center justify-center space-x-2 p-2">
	<div class="flex flex-col space-y-2">
		<Card>
			<div class="flex space-x-2">
				<img src={staticIcons.pspWhite} alt="Palworld Save Pal" class="mb-2" />
				<span class="font-bold">{appState.version ? `v${appState.version}` : ''}</span>
			</div>
			<hr class="border-surface-500" />
			<div class="mt-2 flex flex-col space-y-2">
				<Tooltip position="left" background="bg-transparent">
					Built <s>by a guy who is a developer</s> some guys who are developers for fun.
					{#snippet popup()}
						<img src={Saitama} alt="Saitama" class="inline-block h-48 w-48" />
					{/snippet}
				</Tooltip>
				<div class="flex items-center space-x-2">
					<span>Check out the</span>
					<TooltipButton icon={Github} popupLabel="Github repo">
						<a
							href="https://github.com/oMaN-Rod/palworld-save-pal"
							target="_blank"
							rel="noopener noreferrer"
							class="text-primary-500 hover:underline"><Github /></a
						>
					</TooltipButton>
					<span>for more info.</span>
				</div>
			</div>
		</Card>
		<Card>
			<div class="flex-col space-y-2">
				<h4 class="h4">Shortcuts</h4>
				<div class="grid grid-cols-2">
					<div class="flex items-center">
						<img src={staticIcons.f5Icon} alt="Right Click" class="shortcut-icon" />
						<span class="mx-1">/</span>
						<img src={staticIcons.ctrlIcon} alt="Right Click" class="shortcut-icon" />
						<img src={staticIcons.rIcon} alt="Right Click" class="shortcut-icon" />
					</div>
					<span> Refresh (Purge unsaved changes) </span>
					<div class="flex items-center">
						<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
						<img src={staticIcons.plusIcon} alt="Right Click" class="shortcut-icon" />
						<span class="mx-1">/</span>
						<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
						<img src={staticIcons.minusIcon} alt="Right Click" class="shortcut-icon" />
					</div>
					<span>Zoom in/out</span>
					<div class="flex items-center">
						<img src={staticIcons.rightClickIcon} alt="Right Click" class="shortcut-icon" />
					</div>
					<span>Copy</span>
					<div class="flex items-center">
						<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
						<img src={staticIcons.rightClickIcon} alt="Right Click" class="shortcut-icon" />
					</div>
					<span>Paste</span>
					<div class="flex items-center">
						<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
						<img src={staticIcons.middleClickIcon} alt="Right Click" class="shortcut-icon" />
					</div>
					<span>Delete</span>
					<div class="flex items-center">
						<img src={staticIcons.ctrlIcon} alt="Ctrl" class="shortcut-icon" />
						<img src={staticIcons.leftClickIcon} alt="Left Click" class="shortcut-icon" />
					</div>
					<span>Select</span>
				</div>
			</div>
		</Card>
	</div>
</div>

<style lang="postcss">
	.shortcut-icon {
		height: 32px;
		width: 32px;
	}
</style>
