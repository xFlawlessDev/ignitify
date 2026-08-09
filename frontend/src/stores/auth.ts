import { defineStore } from "pinia";
import { computed, shallowRef } from "vue";
import type { AuthenticatedUser, AuthSession } from "@/lib/types";
import {
  apiBootstrap,
  apiGetMe,
  apiLogin,
  applyAuthSession,
  AUTH_SESSION_CLEARED_EVENT,
  AUTH_SESSION_REFRESHED_EVENT,
  beginLogout,
  clearAuthTokens,
  getToken,
  refreshStoredAccessToken,
} from "@/lib/api";

export const useAuthStore = defineStore("auth", () => {
  const user = shallowRef<AuthenticatedUser | null>(null);
  const token = shallowRef<string | null>(getToken());
  const isInitialized = shallowRef(false);

  const isAuthenticated = computed(() => !!user.value);
  const isPlatformOperator = computed(
    () => user.value?.role === "platform_operator" || user.value?.role === "admin",
  );
  const isAdmin = computed(() => isPlatformOperator.value);
  const currentUser = computed(() => user.value);

  let initPromise: Promise<void> | null = null;

  if (typeof window !== "undefined") {
    window.addEventListener(AUTH_SESSION_REFRESHED_EVENT, (event) => {
      const session = (event as CustomEvent<AuthSession>).detail;
      if (!session) return;
      token.value = session.access_token;
      user.value = session.user;
    });
    window.addEventListener(AUTH_SESSION_CLEARED_EVENT, () => {
      token.value = null;
      user.value = null;
    });
  }

  async function init(): Promise<void> {
    if (initPromise) return initPromise;
    initPromise = doInit();
    return initPromise;
  }

  async function doInit(): Promise<void> {
    if (isInitialized.value) return;
    if (!getToken()) await refreshAccessToken();
    if (getToken()) {
      const result = await apiGetMe();
      if (result.success) user.value = result.data;
      else clearStoredSession();
    }
    isInitialized.value = true;
  }

  async function login(username: string, password: string): Promise<string | null> {
    const result = await apiLogin(username, password);
    if (!result.success) return result.error ?? "Login failed";
    applySession(result.data);
    return null;
  }

  async function bootstrap(
    username: string,
    password: string,
    bootstrapSecret: string,
  ): Promise<string | null> {
    const result = await apiBootstrap(username, password, bootstrapSecret);
    if (!result.success) return result.error ?? "Bootstrap failed";
    applySession(result.data);
    return null;
  }

  async function refreshAccessToken(): Promise<boolean> {
    const session = await refreshStoredAccessToken();
    if (!session) return false;
    token.value = session.access_token;
    user.value = session.user;
    return true;
  }

  function logout(): void {
    beginLogout();
    token.value = null;
    user.value = null;
  }

  function applySession(session: AuthSession): void {
    applyAuthSession(session);
    token.value = session.access_token;
    user.value = session.user;
  }

  function clearStoredSession(): void {
    clearAuthTokens();
    token.value = null;
    user.value = null;
  }

  return {
    user,
    token,
    isInitialized,
    isAuthenticated,
    isPlatformOperator,
    isAdmin,
    currentUser,
    init,
    login,
    bootstrap,
    refreshAccessToken,
    logout,
  };
});
