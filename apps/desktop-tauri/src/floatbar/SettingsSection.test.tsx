import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SettingsSnapshot } from "../types/bridge";
import FloatBarSettingsSection from "./SettingsSection";

vi.mock("../hooks/useLocale", () => ({
  useLocale: () => ({ t: (key: string) => key }),
}));

const settings = {
  floatBarEnabled: true,
  floatBarOpacity: 90,
  floatBarScale: 100,
  floatBarOrientation: "horizontal",
  floatBarStyle: "floating",
  floatBarShowCost: false,
  claudeDailyRoutinesUsageVisible: true,
  claudeAllowReadingClaudeCodeCredentials: false,
  alibabaTokenPlanRegion: "cn",
  weeklyProgressWorkDays: null,
  floatBarShowResetInline: false,
  floatBarDarkText: false,
  floatBarClickThrough: false,
} as unknown as SettingsSnapshot;

describe("FloatBar settings", () => {
  it("applies size presets through the existing scale preference and preserves custom sizes", () => {
    const set = vi.fn();
    const { rerender } = render(<FloatBarSettingsSection settings={{ ...settings, floatBarScale: 125 }} saving={false} set={set} />);
    expect(screen.getByText(/FloatBarCustomSize/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /DisplayModeCompact/ }));
    expect(set).toHaveBeenLastCalledWith({ floatBarScale: 75 });
    rerender(<FloatBarSettingsSection settings={{ ...settings, floatBarScale: 75 }} saving={false} set={set} />);
    expect(screen.getByRole("button", { name: /DisplayModeCompact/ }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: /FloatBarComfortable/ }));
    expect(set).toHaveBeenLastCalledWith({ floatBarScale: 100 });
  });

  it("renders one cost toggle", () => {
    render(
      <FloatBarSettingsSection settings={settings} saving={false} set={vi.fn()} />,
    );

    expect(screen.getAllByText("FloatBarShowCost")).toHaveLength(1);
  });
});
