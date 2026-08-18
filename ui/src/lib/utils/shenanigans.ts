/**
 * Standalone one-liners for the Settings shenanigans button. Stat-less by
 * design — the overview's FunCard builds stat-driven facts; these are the
 * anytime quips that work with or without a save loaded.
 */
const QUIPS = [
	'The Palbox is judging you, and it is not impressed.',
	'Fun fact: your settings are 100% certified shenanigans-free. Until now.',
	'Warning: the Palpagos Islands health inspector has been notified.',
	'This toast has been approved by the Overview Full Mode research division.',
	'Lamball approves of this configuration.',
	'Your settings have been blessed by a passing Cattiva.',
	'Reminder: no pals were harmed in the making of this toast.',
	'Jetragon called. It wants its speed back.',
	'If you tweak these settings too hard, the pals will talk.',
	'Congratulations! Your settings are now 0.01% more suspicious.'
];

export function randomShenanigans(): string {
	return QUIPS[Math.floor(Math.random() * QUIPS.length)] ?? QUIPS[0];
}
