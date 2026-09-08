if (typeof Element !== 'undefined' && !Element.prototype.animate) {
	Element.prototype.animate = function () {
		let onfinish: (() => void) | null = null;
		const animation = {
			cancel: () => {},
			finish: () => {},
			get onfinish() {
				return onfinish;
			},
			set onfinish(handler: (() => void) | null) {
				onfinish = handler;
				handler?.();
			}
		};
		return animation as unknown as Animation;
	};
}
