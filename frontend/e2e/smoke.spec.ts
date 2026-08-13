import { expect, test } from "@playwright/test";
import { installFakeApi } from "./support/fake-api";

async function signIn(page: import("@playwright/test").Page) {
  await page.goto("/login");
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await page.getByLabel("Username").fill("operator");
  await page.getByLabel("Password").fill("correct-horse-battery-staple");
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(/\/dashboard$/);
}

test("bootstraps, refreshes, logs out, and signs in again", async ({ page }) => {
  const state = await installFakeApi(page, { bootstrapRequired: true });
  await page.goto("/login");
  await expect(page.getByRole("heading", { name: "Create administrator" })).toBeVisible();
  await page.getByLabel("Username").fill("operator");
  await page.getByLabel("Bootstrap secret").fill("fixture-bootstrap-secret-that-is-long-enough");
  await page.getByLabel("Password").fill("correct-horse-battery-staple");
  await page.getByRole("button", { name: "Create administrator" }).click();
  await expect(page).toHaveURL(/\/dashboard$/);

  await page.reload();
  await expect(page).toHaveURL(/\/dashboard$/);
  const logoutResponse = page.waitForResponse("**/api/v1/auth/logout");
  await page.getByRole("button", { name: "Sign out" }).click();
  await logoutResponse;
  await expect(page).toHaveURL(/\/login$/);

  await page.getByLabel("Username").fill("operator");
  await page.getByLabel("Password").fill("correct-horse-battery-staple");
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(/\/dashboard$/);
  expect(state.unhandledRequests).toEqual([]);
});

test("creates a service deployment and shows its logs", async ({ page }) => {
  const state = await installFakeApi(page);
  await signIn(page);
  await page.getByRole("link", { name: "Projects", exact: true }).click();
  await page.getByRole("button", { name: "New project" }).click();
  await page.getByLabel("Project name").fill("Smoke project");
  await page.getByRole("button", { name: "Create project" }).click();
  await expect(page).toHaveURL(/\/projects\/project-1$/);

  await page.getByRole("tab", { name: "Services" }).click();
  await page.getByRole("button", { name: "Add service" }).click();
  await page.getByLabel("Service name").fill("web");
  await page.getByRole("button", { name: "Create service" }).click();
  await expect(page).toHaveURL(/\/projects\/project-1\/services\/service-1$/);

  await page.getByRole("button", { name: "Deploy", exact: true }).click();
  await expect(page.getByText("healthy", { exact: true }).first()).toBeVisible();
  await page.getByRole("tab", { name: "Logs", exact: true }).click();
  await expect(page.getByText("deployment fixture complete")).toBeVisible();
  expect(state.unhandledRequests).toEqual([]);
});

test("validates and saves ingress configuration", async ({ page }) => {
  const state = await installFakeApi(page);
  await signIn(page);
  await page.getByRole("link", { name: "Settings" }).click();
  await page.getByRole("tab", { name: "Ingress & TLS" }).click();
  const suffix = page.locator("#application-domain-suffix");
  await suffix.fill("https://invalid.example.test");
  await expect(page.getByRole("button", { name: "Save changes" })).toBeDisabled();
  await suffix.fill("apps.example.test");
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("No unsaved changes")).toBeVisible();
  expect(state.settings.application_domain_suffix).toBe("apps.example.test");
  expect(state.unhandledRequests).toEqual([]);
});

test("keeps privileged routes unavailable to non-operators", async ({ page }) => {
  const anonymousPage = await page.context().newPage();
  const anonymousState = await installFakeApi(anonymousPage);
  await anonymousPage.goto("/settings");
  await expect(anonymousPage).toHaveURL(/\/login\?redirect=(?:%2F|\/)settings$/);
  expect(anonymousState.unhandledRequests).toEqual([]);
  await anonymousPage.close();

  const state = await installFakeApi(page, { role: "user" });
  await signIn(page);
  await page.goto("/settings");
  await expect(page).toHaveURL(/\/dashboard$/);
  expect(state.unhandledRequests).toEqual([]);
});

test("never renders backup credentials returned from configuration", async ({ page }) => {
  const state = await installFakeApi(page);
  await signIn(page);
  await page.getByRole("link", { name: "Settings" }).click();
  await page.getByRole("tab", { name: "Backup" }).click();
  await page.getByLabel("S3 endpoint").fill("https://s3.example.test");
  await page.getByLabel("Region").fill("us-east-1");
  await page.getByLabel("Bucket").fill("ignitify-smoke-backups");
  await page.getByLabel("Access key ID").fill("fixture-access-key");
  await page.getByLabel("Secret access key").fill("fixture-secret-access-key");
  await page.getByRole("button", { name: "Save destination" }).click();
  await expect(page.locator("body")).not.toContainText("fixture-secret-access-key");
  expect(state.backupResponse).not.toHaveProperty("access_key_id");
  expect(state.backupResponse).not.toHaveProperty("secret_access_key");
  expect(state.unhandledRequests).toEqual([]);
});
