<script lang="ts">
	import { RawLayer } from '../raw/index.js';
	import type { CircleLayerProps } from './types.js';

	let { paint = {}, layout = {}, ...rest }: CircleLayerProps = $props();
</script>

<RawLayer type="circle" {paint} {layout} {...rest} />
