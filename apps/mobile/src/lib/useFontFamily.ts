import { Platform } from "react-native";

const FONT_FAMILIES = {
  regular: Platform.select({ ios: "Menlo", default: "monospace" }) ?? "monospace",
  medium: Platform.select({ ios: "Menlo", default: "monospace" }) ?? "monospace",
  bold: Platform.select({ ios: "Menlo-Bold", default: "monospace" }) ?? "monospace",
} as const;

/**
 * Resolves a font family for APIs that require a style object or native prop.
 * Prefer Uniwind font classes when the target component accepts `className`.
 */
export function useFontFamily(weight: keyof typeof FONT_FAMILIES): string {
  return FONT_FAMILIES[weight];
}
