<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
</script>

<div class="animate-fade-in flex w-full flex-col items-center justify-center">
	<h2 class="h2 flex items-center gap-2">
		<Icon icon="tabler:rocket" class="text-secondary-400 h-6 w-6" />
		{m.select_entity_to_edit({ entity: c.player })}
	</h2>
</div>
