<script lang="ts">
	import { Card } from '$components/ui';

	let { closeModal } = $props<{
		closeModal: (value: [string, number]) => void;
	}>();

	function handleLater() {
		closeModal(null);
	}
</script>

<div class="flex h-full w-full flex-col items-center justify-center space-y-4">
	<Card class="min-w-[calc(100vw/3)]">
		<div class="flex flex-col space-y-4 p-4">
			<h3 class="h3">Update Available!</h3>
			<p class="my-4">A new version of Palworld Save Pal is available on GitHub.</p>

			<div class="mt-2 flex flex-row items-center justify-end space-x-2">
				<a
					href="https://github.com/oMaN-Rod/palworld-save-pal/releases"
					target="_blank"
					rel="noopener noreferrer"
					class="btn btn-primary">Update Now</a
				>
				<button class="btn hover:bg-secondary-500" onclick={handleLater}>Later</button>
			</div>
		</div>
	</Card>
</div>
