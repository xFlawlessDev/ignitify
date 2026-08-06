import { readonly, shallowRef } from "vue";

const latestReleaseUrl = "https://api.github.com/repos/xFlawlessDev/ignitify/releases/latest";

interface ReleaseResponse {
  tag_name: string;
  html_url: string;
}

export type AppUpdateResult =
  | { kind: "updateAvailable"; version: string; releaseUrl: string }
  | { kind: "upToDate"; version: string }
  | { kind: "noRelease" }
  | { kind: "failed" };

interface UseAppUpdateOptions {
  currentVersion?: string;
  fetchFn?: typeof fetch;
  releaseUrl?: string;
}

interface SemanticVersion {
  major: number;
  minor: number;
  patch: number;
  prerelease: string[];
}

function parseSemanticVersion(value: string): SemanticVersion | null {
  const match = value
    .trim()
    .replace(/^v/, "")
    .match(/^(\d+)\.(\d+)\.(\d+)(?:-([\w.-]+))?(?:\+.+)?$/);
  if (!match) return null;

  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4]?.split(".") ?? [],
  };
}

function comparePrerelease(left: string[], right: string[]): number {
  if (left.length === 0) return right.length === 0 ? 0 : 1;
  if (right.length === 0) return -1;

  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const leftPart = left[index];
    const rightPart = right[index];
    if (leftPart === undefined) return -1;
    if (rightPart === undefined) return 1;
    if (leftPart === rightPart) continue;

    const leftNumber = Number(leftPart);
    const rightNumber = Number(rightPart);
    const leftIsNumber = Number.isInteger(leftNumber) && /^\d+$/.test(leftPart);
    const rightIsNumber = Number.isInteger(rightNumber) && /^\d+$/.test(rightPart);
    if (leftIsNumber && rightIsNumber) return leftNumber - rightNumber;
    if (leftIsNumber) return -1;
    if (rightIsNumber) return 1;
    return leftPart.localeCompare(rightPart);
  }

  return 0;
}

export function isVersionNewer(latest: string, current: string): boolean {
  const latestVersion = parseSemanticVersion(latest);
  const currentVersion = parseSemanticVersion(current);
  if (!latestVersion || !currentVersion) return false;

  for (const part of ["major", "minor", "patch"] as const) {
    if (latestVersion[part] !== currentVersion[part]) {
      return latestVersion[part] > currentVersion[part];
    }
  }

  return comparePrerelease(latestVersion.prerelease, currentVersion.prerelease) > 0;
}

function isReleaseResponse(value: unknown): value is ReleaseResponse {
  if (!value || typeof value !== "object") return false;
  const release = value as Partial<ReleaseResponse>;
  return typeof release.tag_name === "string" && typeof release.html_url === "string";
}

export function useAppUpdate(options: UseAppUpdateOptions = {}) {
  const appVersion = options.currentVersion ?? __IGNITIFY_APP_VERSION__;
  const fetchFn = options.fetchFn ?? globalThis.fetch.bind(globalThis);
  const releaseUrl = options.releaseUrl ?? latestReleaseUrl;
  const isChecking = shallowRef(false);

  async function checkForUpdate(): Promise<AppUpdateResult> {
    isChecking.value = true;

    try {
      const response = await fetchFn(releaseUrl, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (response.status === 404) return { kind: "noRelease" };
      if (!response.ok) return { kind: "failed" };

      const release: unknown = await response.json();
      if (!isReleaseResponse(release)) return { kind: "failed" };

      const version = release.tag_name.replace(/^v/, "");
      return isVersionNewer(version, appVersion)
        ? { kind: "updateAvailable", version, releaseUrl: release.html_url }
        : { kind: "upToDate", version: appVersion };
    } catch {
      return { kind: "failed" };
    } finally {
      isChecking.value = false;
    }
  }

  return { appVersion, checkForUpdate, isChecking: readonly(isChecking) };
}
