<script lang="ts">
	import OfflineIcon from '@iconify/svelte/dist/OfflineIcon.svelte';
	import './iconCollections';

	let {
		icon,
		size,
		color,
		...rest
	}: {
		/** Iconify icon name, e.g. `tabler:device-floppy` or `svg-spinners:180-ring`. */
		icon: string;
		/** Square size in px (or any CSS length). Sets both width and height. */
		size?: number | string;
		/** CSS color; defaults to `currentColor`. */
		color?: string;
		[key: string]: unknown;
	} = $props();

	// Icons are decorative unless the caller labels them: hide unlabeled icons
	// from assistive tech, and never override an explicit aria-hidden/label.
	const labelled =
		'aria-label' in rest || 'aria-hidden' in rest || 'ariaHidden' in rest || 'role' in rest;
</script>

<OfflineIcon
	{icon}
	width={size}
	height={size}
	{color}
	{...rest}
	aria-hidden={labelled ? undefined : 'true'}
/>
