<script lang="ts">
	let { text, key }: { text: string; key: string } = $props();
</script>

<div class="flex gap-1">
	<span>{text}</span>
	<kbd class="kbd kbd-xs hidden items-center text-center text-xs 2xl:flex">{key}</kbd>
</div>
