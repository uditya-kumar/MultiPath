import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Every page must load without console errors and pass an accessibility scan.
async function open(page: Page, query: string, scheme: "dark" | "light" = "dark") {
  const errors: string[] = [];
  page.on("console", (msg) => msg.type() === "error" && errors.push(msg.text()));
  page.on("pageerror", (err) => errors.push(err.message));
  await page.emulateMedia({ colorScheme: scheme });
  await page.goto(`/demo.html?${query}`);
  await expect(page.getByRole("status", { name: "MultiPath status" }).or(page.getByText("Settings"))).toBeVisible();
  return errors;
}

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).analyze();
  const serious = results.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
  expect(serious.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(", ")}`)).toEqual([]);
}

test("F01 off: explains, offers to turn on, reassures", async ({ page }) => {
  const errors = await open(page, "state=off");
  const hero = page.getByRole("status", { name: "MultiPath status" });
  await expect(hero).toContainText("Off");
  await expect(hero).toContainText("macOS is using Wi-Fi only");
  await expect(page.getByRole("button", { name: "Turn on" })).toBeEnabled();
  await expect(page.getByText("macOS will ask for an administrator password each time")).toBeVisible();
  await expect(page.getByText("Runs only on this Mac: no relay server, no account.")).toBeVisible();
  await axe(page);
  expect(errors).toEqual([]);
});

test("F01 active: combined speed, spread, one row per network", async ({ page }) => {
  const errors = await open(page, "state=active");
  await expect(page.getByRole("status", { name: "MultiPath status" })).toContainText("Active");
  await expect(page.getByRole("list", { name: "Open connections per network" }).getByRole("listitem")).toHaveCount(3);
  for (const name of ["Wi-Fi", "USB 10/100/1000 LAN", "iPhone USB"]) {
    await expect(page.getByRole("switch", { name: `Use ${name}` })).toHaveAttribute("aria-checked", "true");
  }
  await expect(page.getByText("A single download stays on the network it started on")).toBeVisible();
  await axe(page);
  expect(errors).toEqual([]);
});

test("F02/F05 technical details stay behind Advanced, reachable by keyboard", async ({ page }) => {
  await open(page, "state=active");
  const row = page.getByRole("group", { name: /^Wi-Fi:/ });
  const gateway = row.getByText("192.168.178.1", { exact: true });
  await expect(gateway).toBeHidden();
  await row.getByText("Advanced").focus();
  await page.keyboard.press("Enter");
  await expect(gateway).toBeVisible();
  await expect(row.getByText("en0", { exact: true })).toBeVisible();
});

test("F03 limited and needs attention", async ({ page }) => {
  await open(page, "state=limited");
  await expect(page.getByRole("status", { name: "MultiPath status" })).toContainText(
    "iPhone USB has no internet, so new connections go to Wi-Fi.",
  );
  await expect(page.getByRole("group", { name: "iPhone USB: No internet" })).toBeVisible();
  await open(page, "state=attention");
  await expect(page.getByRole("status", { name: "MultiPath status" })).toContainText("No internet");
  await expect(page.getByText("No phone found.")).toBeVisible();
  await axe(page);
});

test("F01-N1 a cancelled password prompt is explained", async ({ page }) => {
  await open(page, "state=failedToStart", "light");
  await expect(page.getByRole("status", { name: "MultiPath status" })).toContainText(
    "the administrator prompt was cancelled",
  );
  await axe(page);
});

test("F04/F06 settings explain themselves; light mode passes contrast", async ({ page }) => {
  const errors = await open(page, "state=off&view=settings", "light");
  await expect(page.getByText("Fixes sites that sign you out")).toBeVisible();
  await expect(page.getByRole("switch", { name: "Open at login" })).toBeVisible();
  await expect(page.getByText("Everything stays on this Mac")).toBeVisible();
  await axe(page);
  expect(errors).toEqual([]);
});

test("keyboard only: Tab reaches the main switch and every network switch", async ({ page }) => {
  await open(page, "state=active");
  const reached = new Set<string>();
  for (let i = 0; i < 25; i++) {
    await page.keyboard.press("Tab");
    const label = await page.evaluate(() => document.activeElement?.getAttribute("aria-label") ?? "");
    if (label) reached.add(label);
  }
  for (const label of ["MultiPath", "Use Wi-Fi", "Use iPhone USB", "Settings", "Connections"]) {
    expect(reached).toContain(label);
  }
});

test("Reduce Motion stops the connection pulse", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/demo.html?state=active");
  const pulse = page.locator(".lane-pulse").first();
  await expect(pulse).toHaveCSS("animation-name", "none");
});

for (const scheme of ["dark", "light"] as const) {
  for (const query of ["state=off", "state=active", "state=limited", "state=attention", "state=failedToStart", "state=active&view=settings"]) {
    test(`accessibility: ${query} in ${scheme} mode`, async ({ page }) => {
      const errors = await open(page, query, scheme);
      await axe(page);
      expect(errors).toEqual([]);
    });
  }
}

test("networks behind one router get an informational note, nothing is blocked", async ({ page }) => {
  const errors = await open(page, "state=sameRouter");
  const note = page.getByRole("note").filter({ hasText: "These connections use the same router" });
  await expect(note).toHaveText(
    "These connections use the same router, so your maximum internet speed may not increase.",
  );
  await expect(page.getByRole("button", { name: "Turn on" })).toBeEnabled();
  for (const name of ["Wi-Fi", "USB 10/100/1000 LAN"]) {
    await expect(page.getByRole("switch", { name: `Use ${name}` })).toBeEnabled();
  }
  await axe(page);
  expect(errors).toEqual([]);
});

test("no router note when each network has its own", async ({ page }) => {
  await open(page, "state=active");
  await expect(page.getByText("use the same router")).toHaveCount(0);
});
