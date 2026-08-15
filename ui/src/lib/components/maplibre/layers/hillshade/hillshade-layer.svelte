<script lang="ts">
	import { RawLayer } from '../raw/index.js';
	import type { HillshadeLayerProps } from './types.js';

	let { paint = {}, layout = {}, ...rest }: HillshadeLayerProps = $props();
</script>

<RawLayer type="hillshade" {paint} {layout} {...rest} />
