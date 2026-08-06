// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { i18n, setLocale } from "./index";

afterEach(() => {
  setLocale("en");
  window.localStorage.clear();
});

describe("i18n locale setup", () => {
  it("changes the global locale, document language, and persisted preference", () => {
    setLocale("id");

    expect(i18n.global.locale.value).toBe("id");
    expect(document.documentElement.lang).toBe("id");
    expect(window.localStorage.getItem("ignitify.locale")).toBe("id");
    expect(i18n.global.t("navigation.overview")).toBe("Ringkasan");
  });
});
