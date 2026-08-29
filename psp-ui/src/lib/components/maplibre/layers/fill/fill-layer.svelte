<script lang="ts">
	import { RawLayer } from '../raw/index.js';
	import type { FillLayerProps } from './types.js';

	let { paint = {}, layout = {}, ...rest }: FillLayerProps = $props();
</script>

<RawLayer type="fill" {paint} {layout} {...rest} />
