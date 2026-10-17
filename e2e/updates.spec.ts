import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// The update flow against the demo page's fake update server (src/demo/fakeUpdater.ts): no request
// ever leaves the machine, which every test checks.
async function open(page: Page, query: string, scheme: "dark" | "light" = "dark") {
  const errors: string[] = [];
  const outside: string[] = [];
  page.on("console", (msg) => msg.type() === "error" && errors.push(msg.text()));
  page.on("pageerror", (err) => errors.push(err.message));
  page.on("request", (req) => !req.url().startsWith("http://localhost:1420/") && outside.push(req.url()));
  await page.emulateMedia({ colorScheme: scheme });
  await page.goto(`/demo.html?${query}`);
  return { errors, outside };
}

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).analyze();
  const serious = results.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
  expect(serious.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(", ")}`)).toEqual([]);
}

const updates = (page: Page) => page.getByRole("region", { name: "Updates" });

test("U01 up to date: check, busy while checking, then says so", async ({ page }) => {
  const { errors, outside } = await open(page, "view=settings&update=latest");
  const panel = updates(page);
  await expect(panel.getByText("MultiPath 1.2.0", { exact: true })).toBeVisible();
  await expect(panel.getByRole("switch", { name: "Check for updates at start" })).toHaveAttribute("aria-checked", "false");
  await panel.getByRole("button", { name: "Check for updates" }).click();
  await expect(panel.getByRole("button", { name: "Checking…" })).toBeDisabled();
  await expect(panel.getByRole("status")).toContainText("You're up to date");
  await expect(panel.getByRole("button", { name: "Check again" })).toBeEnabled();
  await axe(page);
  expect(errors).toEqual([]);
  expect(outside).toEqual([]);
});

test("U02 available: confirm, progress, install, restart", async ({ page }) => {
  const { errors, outside } = await open(page, "view=settings&update=available");
  const panel = updates(page);
  await panel.getByRole("button", { name: "Check for updates" }).click();
  await expect(panel.getByRole("status")).toContainText("MultiPath 1.2.1 is available");
  await expect(panel.getByLabel("What's new in 1.2.1")).toContainText("Faster failover");
  // Nothing happens until the user confirms.
  await page.waitForTimeout(500);
  await expect(panel.getByRole("progressbar")).toHaveCount(0);
  await axe(page);

  await panel.getByRole("button", { name: "Install and restart" }).click();
  const bar = panel.getByRole("progressbar", { name: "Update progress" });
  await expect(bar).toBeVisible();
  await expect(bar).toHaveAttribute("aria-valuenow", /\d+/);
  await expect(panel.getByRole("status")).toContainText("Checking the signature and installing…");
  await expect(panel.getByRole("status")).toContainText("MultiPath 1.2.1 is installed");
  await expect(panel.getByRole("status")).toContainText("Restarting MultiPath…");
  expect(errors).toEqual([]);
  expect(outside).toEqual([]);
});

test("U03 not now: the update is put aside", async ({ page }) => {
  await open(page, "view=settings&update=available");
  const panel = updates(page);
  await panel.getByRole("button", { name: "Check for updates" }).click();
  await panel.getByRole("button", { name: "Not now" }).click();
  await expect(panel.getByRole("button", { name: "Check for updates" })).toBeVisible();
  await expect(panel.getByRole("button", { name: "Install and restart" })).toHaveCount(0);
});

test("U04 a download that fails the signature check is not installed", async ({ page }) => {
  const { outside } = await open(page, "view=settings&update=badSignature", "light");
  const panel = updates(page);
  await panel.getByRole("button", { name: "Check for updates" }).click();
  await panel.getByRole("button", { name: "Install and restart" }).click();
  const alert = panel.getByRole("alert");
  await expect(alert).toContainText("Couldn't install the update");
  await expect(alert).toContainText("didn't pass the security check, so nothing was installed");
  await expect(panel.getByText("MultiPath 1.2.1 is installed")).toHaveCount(0);
  await expect(panel.getByText("github.com/timowse/MultiPath/releases")).toBeVisible();
  await axe(page);
  // Try again starts from a fresh check.
  await panel.getByRole("button", { name: "Try again" }).click();
  await expect(panel.getByRole("status")).toContainText("MultiPath 1.2.1 is available");
  expect(outside).toEqual([]);
});

test("U05 offline: an actionable error, then a retry", async ({ page }) => {
  await open(page, "view=settings&update=offline");
  const panel = updates(page);
  await panel.getByRole("button", { name: "Check for updates" }).click();
  await expect(panel.getByRole("alert")).toContainText("Check that you're online, then try again.");
  await expect(panel.getByRole("button", { name: "Try again" })).toBeEnabled();
  await axe(page);
});

test("U06 restart fails: says how to finish, Restart now retries", async ({ page }) => {
  await open(page, "view=settings&update=restartFails");
  const panel = updates(page);
  await panel.getByRole("button", { name: "Check for updates" }).click();
  await panel.getByRole("button", { name: "Install and restart" }).click();
  await expect(panel.getByRole("alert")).toContainText("Quit MultiPath from the menu bar and open it again");
  await panel.getByRole("button", { name: "Restart now" }).click();
  await expect(panel.getByRole("status")).toContainText("Restarting MultiPath…");
});

test("U07 check at start: points out the update, installs nothing until confirmed", async ({ page }) => {
  const { errors, outside } = await open(page, "state=active&update=available&launch=1");
  const banner = page.getByText("MultiPath 1.2.1 is available.");
  await expect(banner).toBeVisible();
  await axe(page);
  await page.getByRole("button", { name: "Review update" }).click();
  const panel = updates(page);
  await expect(panel.getByRole("button", { name: "Install and restart" })).toBeVisible();
  await expect(panel.getByRole("progressbar")).toHaveCount(0);
  expect(errors).toEqual([]);
  expect(outside).toEqual([]);
});

test("U08 without check at start, nothing is checked", async ({ page }) => {
  await open(page, "state=active&update=available");
  await page.waitForTimeout(600);
  await expect(page.getByText("is available")).toHaveCount(0);
});

test("U09 keyboard only: reach Check for updates and confirm with Enter", async ({ page }) => {
  await open(page, "view=settings&update=available");
  const check = updates(page).getByRole("button", { name: "Check for updates" });
  for (let i = 0; i < 40 && !(await check.evaluate((el) => el === document.activeElement)); i++) {
    await page.keyboard.press("Tab");
  }
  await expect(check).toBeFocused();
  await page.keyboard.press("Enter");
  const install = updates(page).getByRole("button", { name: "Install and restart" });
  await expect(install).toBeVisible();
  await install.focus();
  await page.keyboard.press("Enter");
  await expect(updates(page).getByRole("status")).toContainText("Restarting MultiPath…");
});
