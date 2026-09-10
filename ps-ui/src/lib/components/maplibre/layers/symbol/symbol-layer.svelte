<script lang="ts">
	import { RawLayer } from '../raw/index.js';
	import type { SymbolLayerProps } from './types.js';

	let { paint = {}, layout = {}, ...rest }: SymbolLayerProps = $props();
</script>

<RawLayer type="symbol" {paint} {layout} {...rest} />
