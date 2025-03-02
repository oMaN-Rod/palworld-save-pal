<script lang="ts">
	import { Card } from '$components/ui';
</script>

<div class="flex h-full w-full flex-col items-center justify-center space-y-4">
	<Card class="min-w-96">
		<div class="flex flex-col space-y-4 p-4">
			<h4 class="h4">Update Available!</h4>
			<p>A new version of Palworld Save Pal is available on GitHub.</p>

			<div class="flex items-center justify-center space-x-2">
				<button class="btn variant-filled-primary">Update Now</button>
				<button class="btn variant-ghost">Later</button>
			</div>
		</div>
	</Card>
</div>
