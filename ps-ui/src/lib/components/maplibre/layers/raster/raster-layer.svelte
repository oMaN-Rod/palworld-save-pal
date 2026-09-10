<script lang="ts">
	import { RawLayer } from '../raw/index.js';
	import type { RasterLayerProps } from './types.js';

	let { paint = {}, layout = {}, ...rest }: RasterLayerProps = $props();
</script>

<RawLayer type="raster" {paint} {layout} {...rest} />
