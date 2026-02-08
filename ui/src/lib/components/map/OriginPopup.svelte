<script lang="ts">
	import { Card } from '$components/ui';
	import { mapToWorld, ORIGIN_GAME_X, ORIGIN_GAME_Y } from './utils';
	import { Globe, Map } from 'lucide-svelte';
	import * as m from '$i18n/messages';
</script>

<Card class="min-w-70 shadow-lg">
	<div class="pointer-events-auto space-y-3">
		<div class="border-b pb-3">
			<h3 class="text-foreground text-lg font-bold">{m.origin_point()}</h3>
			<p class="text-muted-foreground text-sm">{m.map_coordinate_origin_point()}</p>
		</div>

		<div class="space-y-2">
			<div class="flex items-start gap-2">
				<Globe class="text-primary mt-0.5 h-4 w-4 shrink-0" />
				<div class="min-w-0 flex-1">
					<div class="text-muted-foreground mb-1 text-xs font-medium">{m.world_coordinates()}</div>
					<div class="text-foreground font-mono text-xs">
						{mapToWorld(ORIGIN_GAME_X, ORIGIN_GAME_Y).x.toFixed(2)}, {mapToWorld(
							ORIGIN_GAME_X,
							ORIGIN_GAME_Y
						).y.toFixed(2)}
					</div>
				</div>
			</div>
			<div class="flex items-start gap-2">
				<Map class="text-primary mt-0.5 h-4 w-4 shrink-0" />
				<div class="min-w-0 flex-1">
					<div class="text-muted-foreground mb-1 text-xs font-medium">{m.map_coordinates()}</div>
					<div class="text-foreground font-mono text-xs">
						{ORIGIN_GAME_X}, {ORIGIN_GAME_Y}
					</div>
				</div>
			</div>
		</div>
	</div>
</Card>
