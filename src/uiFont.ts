/**
 * The interface font stack, for text drawn on a canvas. Keep in step with `--font-ui`
 * in styles.css. `-apple-system` alone is not enough: Chromium-based webviews can
 * resolve it to a font that sets Vietnamese stacked marks (ế, ố) apart from the letter,
 * while `BlinkMacSystemFont` gets San Francisco there.
 */
export const UI_FONT_FAMILY = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif';
