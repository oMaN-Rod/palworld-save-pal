function getComputedColor(varName: string) {
    const computedStyle = getComputedStyle(document.body);
    const value = computedStyle.getPropertyValue(varName).trim();
    return value;
}

export function getComputedColorHex(varName: string): string {
    const color = getComputedColor(varName);
    const hex = rgbToHex(color) as string;
    return hex;
}

function rgbToHex(rgbString: string | undefined) {
    if (!rgbString) return null;
    const rgb = rgbString.split(' ');

    const r = parseInt(rgb[0]);
    const g = parseInt(rgb[1]);
    const b = parseInt(rgb[2]);

    const hexR = componentToHex(r);
    const hexG = componentToHex(g);
    const hexB = componentToHex(b);

    return `#${hexR}${hexG}${hexB}`;
}

function componentToHex(c: number) {
    const hex = c.toString(16);
    return hex.length === 1 ? '0' + hex : hex;
}