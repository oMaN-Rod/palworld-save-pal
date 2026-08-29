<script lang="ts">
	import { RawLayer } from '../raw/index.js';
	import type { LineLayerProps } from './types.js';

	let { paint = {}, layout = {}, ...rest }: LineLayerProps = $props();
</script>

<RawLayer type="line" {paint} {layout} {...rest} />
