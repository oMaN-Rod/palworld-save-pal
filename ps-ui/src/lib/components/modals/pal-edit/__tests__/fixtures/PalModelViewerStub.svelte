<script lang="ts">
	let { characterKey }: { characterKey: string } = $props();
</script>

<div data-testid="pal-model-viewer" data-character-key={characterKey}></div>
