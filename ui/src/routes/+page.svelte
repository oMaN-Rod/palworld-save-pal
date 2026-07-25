<script lang="ts">
	import { getAppState } from '$states';
	import { goto } from '$app/navigation';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';

	const appState = getAppState();
	const desktop = PUBLIC_DESKTOP_MODE === 'true';

	if (!appState.saveFile) {
		goto(desktop ? '/file' : '/upload');
	}
</script>
