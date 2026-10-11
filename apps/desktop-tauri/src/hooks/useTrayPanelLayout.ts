import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { getWorkAreaRect, reanchorTrayPanel, revealTrayPanelWindow } from "../lib/tauri";

export interface TrayPanelLayoutOptions {
  canMeasure: boolean;
  detailMode: boolean;
  layoutKey: string;
}

/** Measure unconstrained content, add columns, then scale only if the screen is full. */
export function useTrayPanelLayout({ canMeasure, detailMode, layoutKey }: TrayPanelLayoutOptions) {
  const [layoutReady, setLayoutReady] = useState(false);
  const [revision, setRevision] = useState(0);
  const timer = useRef<number>();
  const lastSize = useRef("");
  const resizeQueue = useRef(Promise.resolve());
  const requestLayout = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setRevision(value => value + 1), 50);
  }, []);

  useEffect(() => {
    const surface = document.querySelector<HTMLElement>(".menu-surface--tray");
    if (!surface) return;
    const observer = new MutationObserver(requestLayout);
    observer.observe(surface, { childList: true, subtree: true, characterData: true,
      attributes: true, attributeFilter: ["open", "class", "hidden"] });
    surface.addEventListener("load", requestLayout, true);
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let unlistenMove: (() => void) | undefined;
    void getCurrentWindow().onScaleChanged(requestLayout).then(stop => {
      if (cancelled) stop(); else unlisten = stop;
    }).catch(() => {});
    void getCurrentWindow().onMoved(requestLayout).then(stop => {
      if (cancelled) stop(); else unlistenMove = stop;
    }).catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
      unlistenMove?.();
      observer.disconnect();
      surface.removeEventListener("load", requestLayout, true);
      window.clearTimeout(timer.current);
    };
  }, [requestLayout]);

  useEffect(() => {
    if (!canMeasure) return;
    let cancelled = false;
    const resize = async () => {
      const surface = document.querySelector<HTMLElement>(".menu-surface--tray");
      if (!surface) return;
      const win = getCurrentWindow();
      try {
        const [area, dpi] = await Promise.all([
          getWorkAreaRect(), win.scaleFactor(),
        ]);
        if (cancelled) return;
        const maxWidth = Math.max(1, area.width / dpi - 24);
        const maxHeight = Math.max(1, area.height / dpi - 24);
        const count = surface.querySelectorAll(".provider-tile").length;
        const firstColumns = detailMode ? 1 : Math.min(2, Math.max(1, count));
        const maxColumns = detailMode ? 1 : Math.max(firstColumns, Math.min(count, Math.floor((maxWidth - 40) / 200)));
        surface.style.zoom = "1";
        surface.style.height = "auto";
        surface.style.maxHeight = "none";
        let width = 400;
        let height = 0;
        let scale = 1;
        let best = { width, height, scale: 0, columns: firstColumns };
        for (let columns = firstColumns; columns <= maxColumns; columns += 1) {
          width = detailMode ? 440 : 40 + columns * 200 + (columns - 1) * 12;
          surface.style.width = `${width}px`;
          surface.style.setProperty("--tray-columns", String(columns));
          height = Math.ceil(surface.getBoundingClientRect().height) + 2;
          scale = Math.min(1, maxWidth / width, maxHeight / height);
          if (scale > best.scale) best = { width, height, scale, columns };
          if (scale === 1) break;
        }
        surface.style.width = `${best.width}px`;
        surface.style.setProperty("--tray-columns", String(best.columns));
        surface.style.zoom = String(best.scale);
        const targetWidth = Math.ceil(best.width * best.scale);
        const targetHeight = Math.ceil(best.height * best.scale);
        const key = `${targetWidth}:${targetHeight}:${dpi}`;
        if (cancelled) return;
        resizeQueue.current = resizeQueue.current.catch(() => {}).then(async () => {
          if (cancelled) return;
          if (key !== lastSize.current) {
            await win.setSize(new LogicalSize(targetWidth, targetHeight));
            // Even an obsolete in-flight resize changes the native window.
            lastSize.current = key;
            if (!cancelled) await reanchorTrayPanel();
          }
        });
        await resizeQueue.current;
      } catch (error) {
        console.warn("CodexBar automatic tray sizing failed", error);
      }
      if (!cancelled) {
        setLayoutReady(true);
        await Promise.resolve(revealTrayPanelWindow()).catch(() => {});
      }
    };
    const frame = requestAnimationFrame(() => void resize());
    return () => { cancelled = true; cancelAnimationFrame(frame); };
  }, [canMeasure, detailMode, layoutKey, revision]);

  return { layoutReady, requestLayout };
}
