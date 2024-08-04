<script lang="ts">
	let { text } = $props<{ text: string }>();
</script>

<div class="relative inline-block w-full">
	<span class="relative px-2 text-lg font-bold">{text}</span>
	<div
		class="bg-surface-800/50 absolute inset-x-0 bottom-0 -z-10 h-1/2 opacity-5 dark:opacity-50"
	></div>
</div>
