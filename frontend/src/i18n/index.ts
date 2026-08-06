import { createI18n } from "vue-i18n";
import en from "./locales/en";
import id from "./locales/id";

export const supportedLocales = ["en", "id"] as const;
export type Locale = (typeof supportedLocales)[number];

export const localeOptions = [
  { value: "en", labelKey: "languages.en" },
  { value: "id", labelKey: "languages.id" },
] as const satisfies ReadonlyArray<{ value: Locale; labelKey: string }>;

const localeStorageKey = "ignitify.locale";
const messages = { en, id };

function isLocale(value: string | null): value is Locale {
  return value !== null && supportedLocales.includes(value as Locale);
}

function getInitialLocale(): Locale {
  if (typeof window === "undefined") return "en";

  const storedLocale = window.localStorage.getItem(localeStorageKey);
  if (isLocale(storedLocale)) return storedLocale;

  const browserLocales =
    navigator.languages.length > 0 ? navigator.languages : [navigator.language];
  return browserLocales.some((value) => value.toLowerCase().startsWith("id")) ? "id" : "en";
}

function applyDocumentLocale(locale: Locale) {
  if (typeof document !== "undefined") document.documentElement.lang = locale;
}

const initialLocale = getInitialLocale();

export const i18n = createI18n({
  legacy: false,
  locale: initialLocale,
  fallbackLocale: "en",
  messages,
});

applyDocumentLocale(initialLocale);

export function setLocale(locale: Locale) {
  i18n.global.locale.value = locale;
  applyDocumentLocale(locale);

  if (typeof window !== "undefined") window.localStorage.setItem(localeStorageKey, locale);
}

export default i18n;
