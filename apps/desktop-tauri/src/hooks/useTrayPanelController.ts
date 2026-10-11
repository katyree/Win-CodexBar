import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { BootstrapState } from "../types/bridge";
import {
  beginFlyoutGesture,
  dismissTrayPanel,
  endFlyoutGesture,
  openSettingsWindow,
  quitApp as quitApplication,
  reorderProviders,
} from "../lib/tauri";
import { useProviders } from "./useProviders";
import { useSettings } from "./useSettings";
import { useUpdateState } from "./useUpdateState";
import { useLocale } from "./useLocale";
import { useProviderSwitcherKeys } from "./useProviderSwitcherKeys";
import { useSurfaceTarget } from "./useSurfaceMode";
import { useTrayPanelLayout } from "./useTrayPanelLayout";
import type { MenuFooterRow } from "../components/MenuSurface";
import { orderProviderSnapshots } from "../lib/providerOrder";
import {
  hydrateProviderSlots,
  orderedEnabledProviderSlots,
} from "../lib/trayProviders";

const TRAY_INITIAL_REFRESH_DELAY_MS = 250;
const DENSE_OVERVIEW_THRESHOLD = 32;

/**
 * Controller for the tray flyout surface — state, memos, effects, and
 * handlers. JSX stays in `TrayPanel`.
 */
export function useTrayPanelController(state: BootstrapState) {
  const { settings } = useSettings(state.settings);
  const {
    providers,
    isRefreshing,
    refreshingProviderIds,
    refresh,
    hasCachedData,
    hasLoadedCache,
  } = useProviders({
    initialRefreshDelayMs: TRAY_INITIAL_REFRESH_DELAY_MS,
    forceRefreshOnMount: settings.refreshAllProvidersOnMenuOpen,
  });
  const { updateState, checkNow, download, apply, dismiss, openRelease } =
    useUpdateState();

  const { t } = useLocale();
  const surfaceTarget = useSurfaceTarget("trayPanel");

  const sorted = useMemo(
    () =>
      orderProviderSnapshots(
        providers,
        state.providers,
        settings.enabledProviders,
        settings.providerOrder,
      ),
    [providers, settings.enabledProviders, settings.providerOrder, state.providers],
  );
  const denseProviderSlots = useMemo(
    () =>
      orderedEnabledProviderSlots(
        state.providers,
        settings.enabledProviders,
        sorted,
        settings.providerOrder,
      ),
    [settings.enabledProviders, settings.providerOrder, sorted, state.providers],
  );
  const providersById = useMemo(
    () => new Map(sorted.map((provider) => [provider.providerId, provider])),
    [sorted],
  );
  const initialProviderId =
    surfaceTarget?.kind === "provider" ? surfaceTarget.providerId : null;

  // null = overview (all providers), string = single provider detail
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(
    initialProviderId,
  );
  const [gridExpanded, setGridExpanded] = useState(false);
  const expectsDenseOverview =
    selectedProviderId === null &&
    !gridExpanded &&
    settings.enabledProviders.length + 1 > DENSE_OVERVIEW_THRESHOLD;
  const denseTrayProviders = useMemo(() => {
    if (!expectsDenseOverview) return sorted;
    return hydrateProviderSlots(denseProviderSlots, providersById);
  }, [denseProviderSlots, expectsDenseOverview, providersById, sorted]);

  // What the switcher grid displays: the dense overview shows hydrated slots
  // (with placeholders); everything else shows the sorted providers.
  const gridProviders = expectsDenseOverview ? denseTrayProviders : sorted;

  useEffect(() => {
    setSelectedProviderId(initialProviderId);
  }, [initialProviderId]);

  // Cards to display based on mode
  // Overview: all providers in the grid — non-error first, then errors
  // Detail: only the selected provider's card (macOS shows single provider)
  const visibleProviders = useMemo(() => {
    if (selectedProviderId === null) {
      // Overview: show providers in the same Settings/catalog order as the grid.
      return sorted;
    }
    // Detail: show ONLY the selected provider (macOS behavior — no appended errors)
    const match = sorted.find((p) => p.providerId === selectedProviderId);
    if (!match) {
      return sorted;
    }
    return [match];
  }, [denseTrayProviders, sorted, selectedProviderId, gridExpanded]);

  const layoutKey = useMemo(
    () =>
      [
        selectedProviderId ?? "overview",
        gridExpanded ? "expanded" : "collapsed",
        isRefreshing ? "refreshing" : "idle",
        updateState.status,
        updateState.version ?? "",
        updateState.error ?? "",
        expectsDenseOverview ? "dense" : "normal",
        hasLoadedCache ? "cache-ready" : "cache-pending",
        visibleProviders.map((provider) => provider.providerId).join(","),
      ].join("|"),
    [
      selectedProviderId,
      gridExpanded,
      isRefreshing,
      updateState.status,
      updateState.version,
      updateState.error,
      expectsDenseOverview,
      hasLoadedCache,
      visibleProviders,
    ],
  );

  const { layoutReady, requestLayout } = useTrayPanelLayout({
    canMeasure: hasLoadedCache || sorted.length > 0,
    detailMode: selectedProviderId !== null,
    layoutKey,
  });

  const openSettings = useCallback(() => {
    void openSettingsWindow("general").finally(() => {
      void getCurrentWindow().close();
    });
  }, []);
  const openAbout = useCallback(() => {
    void openSettingsWindow("about").finally(() => {
      void getCurrentWindow().close();
    });
  }, []);
  const quitApp = useCallback(() => {
    void quitApplication();
  }, []);

  const footerRows: MenuFooterRow[] = [
    { icon: "↻", label: t("ActionRefresh"), shortcut: "Ctrl+R", onClick: refresh },
    { icon: "⚙", label: t("MenuSettings"), shortcut: "Ctrl+,", onClick: openSettings },
    { icon: "ⓘ", label: t("MenuAbout"), onClick: openAbout },
    { icon: "⌧", label: t("MenuQuit"), shortcut: "Ctrl+Q", onClick: quitApp },
  ];

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (
        e.key === "Escape" &&
        !e.ctrlKey &&
        !e.shiftKey &&
        !e.altKey &&
        !e.metaKey
      ) {
        e.preventDefault();
        void dismissTrayPanel().catch(() => {});
        return;
      }
      if (!e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) return;
      switch (e.key.toLowerCase()) {
        case "r":
          e.preventDefault();
          refresh();
          break;
        case ",":
          e.preventDefault();
          openSettings();
          break;
        case "q":
          e.preventDefault();
          quitApp();
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [refresh, openSettings, quitApp]);

  const handleGridClick = useCallback(
    (providerId: string | null) => {
      setSelectedProviderId(providerId);
    },
    [],
  );
  const gridProviderIds = useMemo(
    () => gridProviders.map((provider) => provider.providerId),
    [gridProviders],
  );
  useProviderSwitcherKeys({
    providerIds: gridProviderIds,
    selectedProviderId,
    onSelect: handleGridClick,
    shortcuts: settings.switcherShortcuts,
  });
  const handleReorder = useCallback((orderedIds: string[]) => {
    void reorderProviders(orderedIds).catch(() => {});
  }, []);
  const handleGestureStart = useCallback(() => {
    void beginFlyoutGesture().catch(() => {});
  }, []);
  const handleGestureEnd = useCallback(() => {
    void endFlyoutGesture().catch(() => {});
  }, []);

  const revealClassName = `tray-panel-reveal${layoutReady ? " tray-panel-reveal--ready" : ""}${expectsDenseOverview ? " tray-panel-reveal--dense" : ""}`;

  return {
    t,
    settings,
    isRefreshing,
    refreshingProviderIds,
    refresh,
    hasCachedData,
    sorted,
    gridProviders,
    selectedProviderId,
    gridExpanded,
    setGridExpanded,
    visibleProviders,
    layoutReady,
    requestLayout,
    footerRows,
    updateState,
    checkNow,
    download,
    apply,
    dismiss,
    openRelease,
    openSettings,
    handleGridClick,
    handleReorder,
    handleGestureStart,
    handleGestureEnd,
    revealClassName,
  };
}
