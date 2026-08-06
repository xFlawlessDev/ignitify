import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { localeOptions, type Locale, setLocale as applyLocale } from "@/i18n";

function isLocale(value: string | undefined): value is Locale {
  return value === "en" || value === "id";
}

export function useLocale() {
  const { locale } = useI18n({ useScope: "global" });
  const currentLocale = computed<Locale>(() => locale.value as Locale);

  function changeLocale(value: string | undefined) {
    if (isLocale(value)) applyLocale(value);
  }

  return {
    currentLocale,
    localeOptions,
    changeLocale,
  };
}
