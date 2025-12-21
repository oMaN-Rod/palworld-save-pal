<script lang="ts">
	import NumberFlow, { NumberFlowGroup } from '@number-flow/svelte'

	type Props = {
		seconds: number
	}

	let { seconds = $bindable() }: Props = $props()

	const hh = $derived(Math.floor(seconds / 3600))
	const mm = $derived(Math.floor((seconds % 3600) / 60))
	const ss = $derived(seconds % 60)
</script>

<NumberFlowGroup>
	<div
		style="font-variant-numeric: tabular-nums; --number-flow-char-height: 0.85em"
		class="text-3xl flex items-baseline font-semibold"
	>
		<NumberFlow value={hh} format={{ minimumIntegerDigits: 2 }} />
		<NumberFlow
			prefix=":"
			value={mm}
			digits={{ 1: { max: 5 } }}
			format={{ minimumIntegerDigits: 2 }}
		/>
		<NumberFlow
			prefix=":"
			value={ss}
			digits={{ 1: { max: 5 } }}
			format={{ minimumIntegerDigits: 2 }}
		/>
	</div>
</NumberFlowGroup>