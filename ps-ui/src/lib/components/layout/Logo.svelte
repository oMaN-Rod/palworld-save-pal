<script lang="ts">
	import { theme, isLightTheme } from '$states';
	import logoWhite from '$lib/assets/img/app/ps_white.webp';
	import logoBlack from '$lib/assets/img/app/ps_black.webp';

	let { class: klass = '' }: { class?: string } = $props();
	const src = $derived(isLightTheme(theme.current) ? logoBlack : logoWhite);
</script>

<img {src} alt="PalStudio" class="h-auto max-w-full {klass}" />
