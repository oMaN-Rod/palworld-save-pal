<script lang="ts">
	// jsdom has no WebGL, so the real viewer's THREE.WebGLRenderer cannot start.
	let { characterKey }: { characterKey: string } = $props();
</script>

<div data-testid="pal-model-viewer" data-character-key={characterKey}></div>
