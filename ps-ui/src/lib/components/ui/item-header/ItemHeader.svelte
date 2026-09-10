<script lang="ts">
	let { text, baseClass = '' } = $props<{ text: string; baseClass?: string }>();
</script>

<div
	class="border-b-surface-800 2xl:border-l-surface-800 border-b-2 2xl:border-b-0 2xl:border-l-2 {baseClass}"
>
	<span class="px-2 text-xs font-bold 2xl:text-base">{text}</span>
</div>
