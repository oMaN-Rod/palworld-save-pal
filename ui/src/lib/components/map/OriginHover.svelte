<script lang="ts">
	import { mapToWorld, ORIGIN_GAME_X, ORIGIN_GAME_Y } from './utils';
	import { Globe, Map } from 'lucide-svelte';
</script>

<div class="popup-content">
	<h3 class="text-lg font-bold">Origin (0,0)</h3>
	<p>This is the origin (0,0) in map coordinates</p>
	<div class="mt-2 space-y-1">
		<div class="flex items-start gap-2">
			<Globe class="text-primary mt-0.5 h-3.5 w-3.5 shrink-0" />
			<div class="min-w-0 flex-1">
				<div class="text-muted-foreground text-xs font-medium">World Coords</div>
				<div class="font-mono text-xs">
					{mapToWorld(ORIGIN_GAME_X, ORIGIN_GAME_Y).x.toFixed(2)}, {mapToWorld(
						ORIGIN_GAME_X,
						ORIGIN_GAME_Y
					).y.toFixed(2)}
				</div>
			</div>
		</div>
		<div class="flex items-start gap-2">
			<Map class="text-primary mt-0.5 h-3.5 w-3.5 shrink-0" />
			<div class="min-w-0 flex-1">
				<div class="text-muted-foreground text-xs font-medium">Map Coords</div>
				<div class="font-mono text-xs">
					{ORIGIN_GAME_X}, {ORIGIN_GAME_Y}
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	.popup-content {
		background-color: var(--color-surface-900);
		color: white;
		padding: 8px;
		border-radius: 4px;
		min-width: 150px;
	}

	.popup-content h3 {
		margin-top: 0;
		margin-bottom: 8px;
	}
</style>
