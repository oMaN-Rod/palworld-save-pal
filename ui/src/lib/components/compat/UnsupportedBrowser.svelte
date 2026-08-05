<script lang="ts">
	import { MonitorDown, TriangleAlert } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	const DESKTOP_URL = 'https://github.com/oMaN-Rod/palworld-save-pal/releases';
</script>

<div class="flex h-screen w-full flex-col items-center justify-center gap-4 px-6 text-center">
	<TriangleAlert class="text-error-400 h-14 w-14" />
	<h1 class="h2 font-bold">{m.compat_unsupported_title()}</h1>
	<p class="text-surface-300 max-w-md">{m.compat_unsupported_body()}</p>
	<a
		href={DESKTOP_URL}
		target="_blank"
		rel="noopener noreferrer"
		class="btn preset-filled-primary-500 mt-2 flex items-center gap-2"
	>
		<MonitorDown size={16} />
		{m.compat_desktop_cta()}
	</a>
</div>
