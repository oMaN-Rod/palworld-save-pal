export type ButtonVariant = 'primary' | 'secondary' | 'danger' | 'neutral' | 'ghost' | 'outline';
export type ButtonSize = 'sm' | 'md' | 'lg' | 'icon';

const VARIANT_CLASS: Record<ButtonVariant, string> = {
	primary: 'btn-primary',
	secondary: 'btn-secondary',
	danger: 'btn-danger',
	neutral: 'btn-neutral',
	ghost: 'btn-ghost',
	outline: 'btn-outline'
};

// 'md' intentionally maps to '' — it uses the base .btn padding.
const SIZE_CLASS: Record<ButtonSize, string> = {
	sm: 'btn-sm',
	md: '',
	lg: 'btn-lg',
	icon: 'btn-icon'
};

export function buttonClasses({
	variant = 'neutral',
	size = 'md'
}: {
	variant?: ButtonVariant;
	size?: ButtonSize;
} = {}): string {
	return ['btn', VARIANT_CLASS[variant], SIZE_CLASS[size]].filter(Boolean).join(' ');
}
