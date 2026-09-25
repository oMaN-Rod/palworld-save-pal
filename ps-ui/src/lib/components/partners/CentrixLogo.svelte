<script lang="ts">
	import { theme, isLightTheme } from '$states';
	import logoWhite from '$lib/assets/img/partners/centrixnodes_white.png';
	import logoBlack from '$lib/assets/img/partners/centrixnodes_black.png';

	let { class: klass = '' }: { class?: string } = $props();
	const src = $derived(isLightTheme(theme.current) ? logoBlack : logoWhite);
</script>

<img {src} alt="CentrixNodes" width="1000" height="155" class="h-auto max-w-full {klass}" />
