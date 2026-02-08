<script lang="ts">
	import { Card } from '$components/ui';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

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
			<h3 class="h3">{m.update_available()}</h3>
			<p class="my-4">{m.new_version_available()}</p>

			<div class="mt-2 flex flex-row items-center justify-end space-x-2">
				<a
					href="https://github.com/oMaN-Rod/palworld-save-pal/releases"
					target="_blank"
					rel="noopener noreferrer"
					class="btn btn-primary">{m.update_now()}</a
				>
				<button class="btn hover:bg-secondary-500" onclick={handleLater}>{m.later()}</button>
			</div>
		</div>
	</Card>
</div>
