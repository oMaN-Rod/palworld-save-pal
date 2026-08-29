<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';

	let { saveHints = false, touch = false }: { saveHints?: boolean; touch?: boolean } = $props();
</script>

{#snippet hint(icons: string[], text: string)}
	<div class="flex items-center gap-2">
		<span class="flex shrink-0 items-center gap-0.5">
			{#each icons as icon (icon)}
				<img src={icon} alt="" class="h-6 w-6" />
			{/each}
		</span>
		<span class="text-surface-500 text-xs">{text}</span>
	</div>
{/snippet}

{#snippet touchHint(icon: string, text: string)}
	<div class="flex items-center gap-2">
		<Icon {icon} class="text-surface-500 h-5 w-5 shrink-0" />
		<span class="text-surface-500 text-xs">{text}</span>
	</div>
{/snippet}

<div class="mt-auto flex flex-col gap-2">
	{#if touch}
		<p class="text-surface-500 text-sm">{m.map_hint_3d_toggle()}</p>
		<div class="flex flex-col gap-1">
			{@render touchHint('tabler:hand-finger', m.map_hint_tap_marker())}
			{#if saveHints}
				{@render touchHint('tabler:target', m.map_hint_tap_focus())}
				{@render touchHint('tabler:toggle-left', m.map_hint_tap_toggle_point())}
			{/if}
			{@render touchHint('tabler:rotate-360', m.map_hint_touch_rotate())}
			{@render touchHint('tabler:zoom-in', m.map_hint_pinch_zoom())}
		</div>
	{:else}
		<p class="text-surface-500 text-sm">{m.click_map_coordinates()}</p>
		<p class="text-surface-500 text-sm">{m.map_hint_3d_toggle()}</p>
		<div class="flex flex-col">
			{#if saveHints}
				{@render hint([staticIcons.leftClickIcon], m.left_click_focus())}
				{@render hint([staticIcons.leftClickIcon], m.click_toggle_point())}
				{@render hint([staticIcons.rightClickIcon], m.right_click_edit_base())}
			{/if}
			<!-- Right-drag and ctrl+left-drag are the two bindings MapLibre's rotate and
			     pitch handlers accept; both are live only while 3D is on, since
			     dragRotate follows it. -->
			{@render hint([staticIcons.rightClickIcon], m.map_hint_rotate_tilt())}
			{@render hint([staticIcons.ctrlIcon, staticIcons.leftClickIcon], m.map_hint_ctrl_rotate())}
			{@render hint([staticIcons.middleClickIcon], m.map_hint_scroll_zoom())}
		</div>
	{/if}
</div>
