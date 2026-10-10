import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { searchSettings } from "./settingsSearch";
import type { LocaleKey } from "../../i18n/keys";

const english = Object.fromEntries([...readFileSync("../../rust/src/locale/en-US.ftl", "utf8").matchAll(/^(\w+) = (.*)$/gm)].map(match => [match[1], match[2]]));
const translate = (key: LocaleKey) => english[key] ?? key;

describe("Settings search", () => {
  it("finds the floating section across whitespace and case, and keeps tray icon results on their own page", () => {
    expect(searchSettings("  FLOATING   size  ", translate).map(target => target.menuSection)).toEqual(["floating"]);
    expect(searchSettings("stacked", translate).map(target => target.tab)).toEqual(["menuBar"]);
    expect(searchSettings("theme", translate).map(target => target.tab)).toContain("general");
    expect(searchSettings("", translate)).toEqual([]);
    expect(searchSettings("no-such-setting", translate)).toEqual([]);
  });
  it("searches translated labels and provider names", () => {
    expect(searchSettings("Apariencia", key => key === "ThemeLabel" ? "Apariencia" : "").map(target => target.tab)).toEqual(["general"]);
    expect(searchSettings("Test Provider", translate, ["Test Provider"]).map(target => target.tab)).toEqual(["providers"]);
  });
});
