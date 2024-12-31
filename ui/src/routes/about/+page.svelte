<script lang="ts">
	import { getAppState } from '$states';
	import { Card, TooltipButton } from '$components/ui';
	import { Github } from 'lucide-svelte';
	import Saitama from '$lib/assets/img/app/saitama.png';
	import { staticIcons } from '$lib/constants';

	const appState = getAppState();
</script>

<div class="flex h-full w-full items-center justify-center space-x-2 p-2">
	<div class="flex w-1/3 flex-col space-y-2">
		<Card class="max-w-lg">
			<div class="flex space-x-2">
				<h1 class="h1">Palworld Save Pal</h1>
				<span class="font-bold">v{appState.version || 'Loading...'}</span>
			</div>
			<hr class="border-surface-500" />
			<div class="flex flex-col space-y-2">
				<p class="text-lg">
					Built by a guy who is a developer for fun.
					<img src={Saitama} alt="Saitama" class="inline-block h-16 w-16" />
				</p>
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
		<Card class="max-w-lg">
			<div class="flex-col space-y-2">
				<h4 class="h4">Shortcuts</h4>

				<h6 class="h6">Keyboard</h6>
				<table class="w-full">
					<tbody>
						<tr>
							<td>
								<div class="flex items-center">
									<img src={staticIcons.f5Icon} alt="Right Click" class="shortcut-icon" />
									<span class="mx-1">/</span>
									<img src={staticIcons.ctrlIcon} alt="Right Click" class="shortcut-icon" />
									<img src={staticIcons.rIcon} alt="Right Click" class="shortcut-icon" />
								</div>
							</td>
							<td>Refresh (Purge unsaved changes)</td>
						</tr>
						<tr>
							<td>
								<div class="flex items-center">
									<img src={staticIcons.ctrlIcon} alt="Right Click" class="shortcut-icon" />
									<img src={staticIcons.plusIcon} alt="Right Click" class="shortcut-icon" />
									<span class="mx-1">/</span>
									<img src={staticIcons.ctrlIcon} alt="Right Click" class="shortcut-icon" />
									<img src={staticIcons.minusIcon} alt="Right Click" class="shortcut-icon" />
								</div>
							</td>
							<td>Zoom in/out</td>
						</tr>
					</tbody>
				</table>
				<hr class="border-surface-500" />
				<h6 class="h6">Mouse</h6>
				<table class="w-full">
					<tbody>
						<tr>
							<td class="font-bold">
								<img src={staticIcons.rightClickIcon} alt="Right Click" class="shortcut-icon" />
							</td>
							<td>Copy</td>
						</tr>
						<tr>
							<td class="font-bold">
								<div class="flex">
									<img src={staticIcons.ctrlIcon} alt="Right Click" class="shortcut-icon" />
									<img src={staticIcons.rightClickIcon} alt="Right Click" class="shortcut-icon" />
								</div>
							</td>
							<td>Paste</td>
						</tr>
						<tr>
							<td>
								<div class="flex">
									<img src={staticIcons.ctrlIcon} alt="Right Click" class="shortcut-icon" />
									<img src={staticIcons.middleClickIcon} alt="Right Click" class="shortcut-icon" />
								</div>
							</td>
							<td>Delete</td>
						</tr>
					</tbody>
				</table>
			</div>
		</Card>
	</div>
</div>

<style lang="postcss">
	.shortcut-icon {
		@apply h-8 w-8;
	}
</style>
