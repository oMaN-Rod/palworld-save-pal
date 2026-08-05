<script lang="ts">
	import { theme, isLightTheme } from '$states';
	import logoWhite from '$lib/assets/img/app/psp_white.webp';
	import logoBlack from '$lib/assets/img/app/psp_black.webp';

	let { class: klass = '' }: { class?: string } = $props();
	const src = $derived(isLightTheme(theme.current) ? logoBlack : logoWhite);
</script>

<img {src} alt="Palworld Save Pal" class="h-auto max-w-full {klass}" />
