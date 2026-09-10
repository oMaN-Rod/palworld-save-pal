<script lang="ts">
	import { RawLayer } from '../raw/index.js';
	import type { FillExtrusionLayerProps } from './types.js';

	let { paint = {}, layout = {}, ...rest }: FillExtrusionLayerProps = $props();
</script>

<RawLayer type="fill-extrusion" {paint} {layout} {...rest} />
