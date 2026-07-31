// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const originalTheme = document.documentElement.className;

afterEach(() => {
  window.localStorage.clear();
  document.documentElement.className = originalTheme;
  vi.resetModules();
});

describe("useControlPlanePreferences", () => {
  it("persists theme and sidebar choices", async () => {
    const { useControlPlanePreferences } = await import("./useControlPlanePreferences");
    const preferences = useControlPlanePreferences();

    preferences.toggleSidebar();
    preferences.toggleTheme();

    expect(preferences.isSidebarCollapsed.value).toBeTruthy();
    expect(preferences.isDark.value).toBeTruthy();
    expect(window.localStorage.getItem("ignitify.sidebar-collapsed")).toBe("true");
    expect(window.localStorage.getItem("ignitify.theme")).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBeTruthy();
  });
});
