import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { JetBrainsCreds } from "./JetBrainsCreds";

const mocks = vi.hoisted(() => ({
  listJetbrainsDetectedIdes: vi.fn(),
  setJetbrainsIdePath: vi.fn(),
  refreshProviders: vi.fn(),
  openPath: vi.fn(),
}));
vi.mock("../../../../../lib/tauri", () => mocks);

describe("JetBrains usage source", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.setJetbrainsIdePath.mockResolvedValue(undefined);
    mocks.refreshProviders.mockResolvedValue(undefined);
  });

  it("shows the live CLI source while retaining an invalid saved fallback path", async () => {
    mocks.listJetbrainsDetectedIdes.mockResolvedValue([
      { id: "central-cli", displayName: "JetBrains Central CLI", path: "C:/Test/central.exe", detected: true, selected: true, isCustom: false, source: "cli" },
      { id: "custom", displayName: "logs", path: "C:/Test/logs", detected: false, selected: false, isCustom: true, source: "local" },
    ]);
    render(<JetBrainsCreds t={key => key} />);
    expect(await screen.findByText("JetBrains Central CLI: C:/Test/central.exe")).toBeTruthy();
    expect(screen.getByRole("textbox")).toHaveValue("C:/Test/logs");
    expect(screen.queryByText(/CredsJetBrainsHelperDetectedPrefix/)).toBeNull();
  });

  it("clears the saved override and refreshes usage after saving", async () => {
    mocks.listJetbrainsDetectedIdes.mockResolvedValue([
      { id: "custom", displayName: "CustomIDE", path: "C:/Test/CustomIDE", detected: true, selected: true, isCustom: true, source: "local" },
    ]);
    render(<JetBrainsCreds t={key => key} />);
    await waitFor(() => expect(screen.getByRole("textbox")).toHaveValue("C:/Test/CustomIDE"));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "" } });
    mocks.listJetbrainsDetectedIdes.mockResolvedValue([]);
    fireEvent.click(screen.getByRole("button", { name: "CredsSavePathAction" }));
    await waitFor(() => expect(mocks.refreshProviders).toHaveBeenCalledOnce());
    expect(mocks.setJetbrainsIdePath).toHaveBeenCalledWith("");
    await waitFor(() => expect(screen.getByRole("textbox")).toHaveValue(""));
  });
});
