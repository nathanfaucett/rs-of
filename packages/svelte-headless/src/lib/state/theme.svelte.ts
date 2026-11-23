import { BROWSER } from "esm-env";

export type ThemeType = "dark" | "light";

let preference = $state<ThemeType>(
  BROWSER && window.matchMedia("(prefers-color-scheme: dark)")?.matches ? "dark" : "light"
);
let theme = $state<ThemeType | undefined>();

export function setTheme(newTheme: ThemeType) {
  theme = newTheme;
}

export function resetTheme() {
  theme = undefined;
}

export function getTheme() {
  return theme ?? preference;
}

if (BROWSER) {
  if (typeof window !== "undefined" && window.matchMedia) {
    const mediaQueryList = window.matchMedia("(prefers-color-scheme: dark)");

    function handleColorSchemeChange(event: MediaQueryListEvent | MediaQueryList) {
      preference = event.matches ? "dark" : "light";
    }

    mediaQueryList.addEventListener("change", handleColorSchemeChange);

    handleColorSchemeChange(mediaQueryList);
  }
}
