import { computed, shallowRef } from "vue";

const sidebarStorageKey = "ignitify.sidebar-collapsed";
const themeStorageKey = "ignitify.theme";

function readBoolean(key: string, fallback: boolean) {
  const value = window.localStorage.getItem(key);
  return value === "true" ? true : value === "false" ? false : fallback;
}

function readTheme() {
  return window.localStorage.getItem(themeStorageKey) === "dark" ? "dark" : "light";
}

const isSidebarCollapsed = shallowRef(readBoolean(sidebarStorageKey, false));
const theme = shallowRef<"light" | "dark">(readTheme());

function applyTheme() {
  document.documentElement.classList.toggle("dark", theme.value === "dark");
}

applyTheme();

export function useControlPlanePreferences() {
  const isDark = computed(() => theme.value === "dark");

  function toggleSidebar() {
    isSidebarCollapsed.value = !isSidebarCollapsed.value;
    window.localStorage.setItem(sidebarStorageKey, String(isSidebarCollapsed.value));
  }

  function toggleTheme() {
    theme.value = theme.value === "dark" ? "light" : "dark";
    window.localStorage.setItem(themeStorageKey, theme.value);
    applyTheme();
  }

  return {
    isDark,
    isSidebarCollapsed,
    toggleSidebar,
    toggleTheme,
  };
}
