<script lang="ts">
	import { Button, Card } from '$components/ui';
	import * as m from '$i18n/messages';
	import { openExternalLink } from '$lib/utils/externalLink';

	const RELEASES_URL = 'https://github.com/oMaN-Rod/palworld-save-pal/releases';

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
				<Button
					href={RELEASES_URL}
					variant="primary"
					target="_blank"
					rel="noopener noreferrer"
					onclick={(event: MouseEvent) => openExternalLink(event, RELEASES_URL)}
					>{m.update_now()}</Button
				>
				<Button variant="ghost" onclick={handleLater}>{m.later()}</Button>
			</div>
		</div>
	</Card>
</div>
