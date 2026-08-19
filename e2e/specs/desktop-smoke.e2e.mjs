import assert from "node:assert/strict";

describe("RemoteOpsX packaged desktop smoke", () => {
  it("boots the real Tauri window into the operations workspace", async () => {
    const brand = await $(".brand");
    await brand.waitForDisplayed();
    assert.match(await brand.getText(), /RemoteOpsX/);

    const dashboard = await $(".operations-dashboard");
    await dashboard.waitForDisplayed();
    const heading = await dashboard.$("h1");
    assert.ok((await heading.getText()).length > 0);
  });

  it("opens the universal palette and indexes operator actions", async () => {
    const trigger = await $(".command-trigger");
    await trigger.click();

    const palette = await $("[aria-label='Command palette']");
    await palette.waitForDisplayed();
    const input = await palette.$("input");
    await input.setValue("Runbook Studio");

    const studioAction = await palette.$("button*=Open Runbook Studio");
    await studioAction.waitForDisplayed();
    assert.match(await studioAction.getText(), /Runbook Studio/);
    await browser.keys("Escape");
    await palette.waitForDisplayed({ reverse: true });
  });

  it("validates a runbook through the Rust backend without opening SSH", async () => {
    await $("button=Studio").click();
    const studio = await $("[aria-label='Runbook Studio']");
    await studio.waitForDisplayed();

    await studio.$("button=Validate / Dry run").click();
    const valid = await studio.$(".warn-banner.ok");
    await valid.waitForDisplayed();
    assert.match(await valid.getText(), /Valid/);

    const steps = await studio.$$(".studio-step");
    assert.ok(steps.length >= 1, "expected the backend preview to return runbook steps");
    await studio.$("button=Close").click();
    await studio.waitForDisplayed({ reverse: true });
  });

  it("persists settings through real Tauri IPC and SQLite", async () => {
    await $("button=Settings").click();
    const dialog = await $(".settings-modal");
    await dialog.waitForDisplayed();

    const theme = await dialog.$("#settings-theme");
    const original = await theme.getValue();
    const target = original === "nord" ? "dracula" : "nord";
    await theme.selectByAttribute("value", target);
    await dialog.$("button=Save settings").click();
    await dialog.waitForDisplayed({ reverse: true });

    await $("button=Settings").click();
    const reopened = await $(".settings-modal");
    await reopened.waitForDisplayed();
    assert.equal(await reopened.$("#settings-theme").getValue(), target);
    await reopened.$("[aria-label='Close settings']").click();
  });
});
