import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
const native = vi.hoisted(() => ({ setSize: vi.fn().mockResolvedValue(undefined),
  scaleFactor: vi.fn().mockResolvedValue(1), onMoved: vi.fn().mockResolvedValue(() => {}), onScaleChanged: vi.fn().mockResolvedValue(() => {}) }));
const bridge = vi.hoisted(() => ({ getWorkAreaRect: vi.fn(),
  reanchorTrayPanel: vi.fn().mockResolvedValue(undefined), revealTrayPanelWindow: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => native,
  LogicalSize: class { constructor(public width: number, public height: number) {} } }));
vi.mock("../lib/tauri", () => bridge);
import { useTrayPanelLayout } from "./useTrayPanelLayout";
let surface: HTMLElement;
let contentRows = 4;
beforeEach(() => {
  vi.clearAllMocks();
  native.onMoved.mockResolvedValue(() => {});
  native.onScaleChanged.mockResolvedValue(() => {});
  native.setSize.mockResolvedValue(undefined);
  bridge.reanchorTrayPanel.mockResolvedValue(undefined);
  bridge.revealTrayPanelWindow.mockResolvedValue(undefined);
  native.scaleFactor.mockResolvedValue(1);
  bridge.getWorkAreaRect.mockResolvedValue({ width: 1280, height: 900 });
  document.body.innerHTML = '<div class="menu-surface--tray">' + '<button class="provider-tile"></button>'.repeat(8) + '</div>';
  surface = document.querySelector<HTMLElement>(".menu-surface--tray")!;
  contentRows = 4;
  vi.spyOn(surface, "getBoundingClientRect").mockImplementation(() => {
    const columns = Number(surface.style.getPropertyValue("--tray-columns")) || 2;
    return { height: Math.ceil(contentRows * 2 / columns) * 230 + 150 } as DOMRect;
  });
});
afterEach(() => { vi.restoreAllMocks(); document.body.innerHTML = ""; });
it("adds columns so every provider fits without scrolling and shrinks when content is removed", async () => {
  const { result, unmount } = renderHook(() => useTrayPanelLayout({ canMeasure: true, detailMode: false, layoutKey: "eight" }));
  await waitFor(() => expect(result.current.layoutReady).toBe(true));
  expect(surface.style.getPropertyValue("--tray-columns")).toBe("3");
  expect(surface.style.zoom).toBe("1");
  expect(native.setSize).toHaveBeenLastCalledWith(expect.objectContaining({ width: 664, height: 842 }));
  contentRows = 1;
  act(() => result.current.requestLayout());
  await waitFor(() => expect(native.setSize).toHaveBeenLastCalledWith(expect.objectContaining({ width: 452, height: 382 })));
  unmount();
});
it("fits oversized content inside the usable area at 200 percent Windows DPI", async () => {
  native.scaleFactor.mockResolvedValue(2);
  bridge.getWorkAreaRect.mockResolvedValue({ width: 1280, height: 900 });
  const { result, unmount } = renderHook(() => useTrayPanelLayout({ canMeasure: true, detailMode: false, layoutKey: "small" }));
  await waitFor(() => expect(result.current.layoutReady).toBe(true));
  const size = native.setSize.mock.calls[native.setSize.mock.calls.length - 1]?.[0];
  expect(size.width).toBeLessThanOrEqual(616);
  expect(size.height).toBeLessThanOrEqual(426);
  expect(Number(surface.style.zoom)).toBeLessThan(1);
  unmount();
});

it("corrects an obsolete in-flight native resize when the content returns to its previous size", async () => {
  contentRows = 1;
  const { result, unmount } = renderHook(() => useTrayPanelLayout({ canMeasure: true, detailMode: false, layoutKey: "race" }));
  await waitFor(() => expect(result.current.layoutReady).toBe(true));
  let finishResize: () => void = () => {};
  native.setSize.mockImplementationOnce(() => new Promise<void>(resolve => { finishResize = resolve; }));
  contentRows = 2;
  act(() => result.current.requestLayout());
  await waitFor(() => expect(native.setSize).toHaveBeenCalledTimes(2));
  contentRows = 1;
  act(() => result.current.requestLayout());
  await act(async () => { await new Promise(resolve => setTimeout(resolve, 90)); });
  await act(async () => finishResize());
  await waitFor(() => expect(native.setSize).toHaveBeenCalledTimes(3));
  expect(native.setSize).toHaveBeenLastCalledWith(expect.objectContaining({ width: 452, height: 382 }));
  unmount();
});
