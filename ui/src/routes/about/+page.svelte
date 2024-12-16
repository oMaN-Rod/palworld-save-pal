<script lang="ts">
	import { getAppState } from '$states';
	import { Card, TooltipButton } from '$components/ui';
	import { Github } from 'lucide-svelte';
	import Saitama from '$lib/assets/img/app/saitama.png';

	const appState = getAppState();
</script>

<div class="flex h-full w-full items-center justify-center space-x-2 p-2">
	<div class="flex w-1/3 flex-col">
		<Card class="max-w-lg">
			<div class="flex space-x-2">
				<h1 class="h1">Palworld Save Pal</h1>
				<span class="font-bold">v{appState.version || 'Loading...'}</span>
			</div>

			<div class="flex flex-col space-y-2">
				<hr class="border-surface-500" />
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
	</div>
	<!-- <Card class="w-full">Details</Card> -->
</div>
