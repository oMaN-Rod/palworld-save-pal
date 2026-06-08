<script lang="ts">
	import { Tooltip, Button } from '$components/ui';
	import type { ButtonVariant, ButtonSize } from '../button/button.styles';

	let {
		children,
		buttonClass = '',
		popupLabel,
		position = 'bottom',
		baseClass = '',
		variant = 'ghost',
		size = 'icon',
		...additionalProps
	}: {
		children: any;
		buttonClass?: string;
		popupLabel: string;
		baseClass?: string;
		position?: 'top' | 'bottom' | 'left' | 'right';
		variant?: ButtonVariant;
		size?: ButtonSize;
		[key: string]: any;
	} = $props();
</script>

<Tooltip {position} {baseClass}>
	<Button {variant} {size} class={buttonClass} {...additionalProps}>
		{@render children()}
	</Button>
	{#snippet popup()}
		{popupLabel}
	{/snippet}
</Tooltip>
