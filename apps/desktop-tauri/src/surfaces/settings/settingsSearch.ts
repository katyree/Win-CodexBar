import type { LocaleKey } from "../../i18n/keys";
import type { SettingsTabId } from "../../types/bridge";
import type { MenuSection } from "./tabs/DisplayTab";

export type SettingsSearchTarget = {
  tab: SettingsTabId;
  label: LocaleKey;
  keywords: LocaleKey[];
  menuSection?: MenuSection;
};

// Search only UI labels and help text, never account values or credentials.
export const SETTINGS_SEARCH_TARGETS: SettingsSearchTarget[] = [
  { tab: "general", label: "TabGeneral", keywords: ["ThemeAutoOption", "ThemeLightOption", "ThemeDarkOption", "InterfaceLanguage", "PreferredCurrencyLabel", "PreferredCurrencyHelper", "ThemeLabel", "ThemeHelper", "StartAtLogin", "StartAtLoginHelper", "StartMinimized", "StartMinimizedHelper", "RefreshIntervalLabel", "RefreshIntervalHelper", "RefreshAllProvidersOnMenuOpen", "RefreshAllProvidersOnMenuOpenHelper", "LowPowerMode", "LowPowerModeHelper"] },
  { tab: "providers", label: "TabProviders", keywords: ["HidePersonalInfo", "TabApiKeys", "TabCookies"] },
  { tab: "notifications", label: "TabNotifications", keywords: ["ShowNotifications", "ShowNotificationsHelper", "PredictivePaceWarnings", "PredictivePaceWarningsHelper", "CredentialExpiryNotifications", "CredentialExpiryNotificationsHelper", "SoundEnabled", "SoundEnabledHelper", "NotificationSoundTheme", "NotificationSoundThemeHelper", "HighUsageAlert", "HighUsageWarningHelper", "CriticalUsageAlert", "CriticalUsageWarningHelper"] },
  { tab: "menuBar", label: "TabMenuBar", keywords: ["TrayIconModeStacked", "TrayIconModeSingle", "TrayIconModePerProvider", "TrayIconModeLabel", "TrayIconModeHelper", "StackedTrayTopProvider", "StackedTrayBottomProvider", "ShowProviderIcons", "ShowProviderIconsHelper", "PreferHighestUsage", "PreferHighestUsageHelper", "ShowPercentInTray", "ShowPercentInTrayHelper", "ColorPaceInTray", "ColorPaceInTrayHelper", "DisplayModeLabel", "DisplayModeHelper", "PromoteTrayIconLabel"] },
  { tab: "menu", label: "SettingsMenuTray", menuSection: "tray", keywords: ["TrayPanelAlwaysOnTopLabel", "TrayPanelAlwaysOnTopHelper", "ShowAsUsedLabel", "ShowAsUsedHelper", "OverviewLayoutLabel", "OverviewLayoutHelper", "ShowAllTokenAccountsLabel", "ShowAllTokenAccountsHelper", "ResetTimeRelative", "ResetTimeRelativeHelper", "ShowResetWhenExhausted", "ShowResetWhenExhaustedHelper", "ShowPace", "ShowPaceHelper"] },
  { tab: "menu", label: "FloatBarSectionTitle", menuSection: "floating", keywords: ["FloatBarShowFloatingBar", "FloatBarShowFloatingBarHelper", "FloatBarOrientation", "FloatBarOrientationHelper", "FloatBarStyle", "FloatBarStyleHelper", "FloatBarOpacityHelper", "FloatBarOpacityAriaLabel", "FloatBarSizePreset", "FloatBarSizePresetHelper", "FloatBarSizeHelper", "FloatBarSizeAriaLabel", "FloatBarShowCost", "FloatBarShowCostDescription", "FloatBarShowResetInline", "FloatBarShowResetInlineHelper", "FloatBarInvertColors", "FloatBarInvertColorsHelper", "FloatBarClickThrough", "FloatBarClickThroughHelper", "FloatBarSize", "FloatBarOpacity", "DisplayModeCompact", "FloatBarComfortable"] },
  { tab: "menu", label: "SettingsMenuShortcuts", menuSection: "shortcuts", keywords: ["SwitcherShortcutPrevious", "SwitcherShortcutNext", "SwitcherShortcutSelect", "SwitcherShortcutsTitle", "SwitcherShortcutsHelper", "SwitcherShortcutRecordingHint", "SwitcherShortcutNone", "SwitcherShortcutReset"] },
  { tab: "usageSpend", label: "TabUsageSpend", keywords: ["UsageSpendSpend", "UsageSpendPriceCoverage", "UsageSpendConversations", "UsageSpendTokenMix"] },
  { tab: "advanced", label: "TabAdvanced", keywords: ["GlobalShortcutFieldLabel", "GlobalShortcutToggleHelper", "CodexLogPathsLabel", "CodexLogPathsHelper", "AgentSessionsEnableLabel", "AgentSessionsEnableHelper", "AgentSessionsStayAwakeLabel", "AgentSessionsStayAwakeHelper", "AgentSessionsSshHostsLabel", "AgentSessionsSshHostsHelper", "HidePersonalInfo", "HidePersonalInfoHelper", "PowerToysPipeLabel", "PowerToysPipeHelper", "NetworkProxyEnableLabel", "NetworkProxyEnableHelper", "NetworkProxyUrlLabel", "NetworkProxyUrlHelper", "NetworkProxyUserLabel", "NetworkProxyPasswordLabel", "NetworkProxyPasswordHelper", "HooksEnableLabel", "HooksEnableHelper", "DisableAllKeychainLabel", "DisableAllKeychainHelper", "AvoidKeychainPromptsLabel", "AvoidKeychainPromptsHelper"] },
  { tab: "about", label: "TabAbout", keywords: ["AboutLoading", "AboutCopyrightBefore", "AboutCopyrightAfter", "AppName", "Version", "SubmitIssue", "ErrorPrefix", "AutoDownloadUpdates", "AutoDownloadUpdatesHelper", "UpdateChannelChoice", "UpdateChannelStableOption", "UpdateChannelBetaOption", "UpdateChannelChoiceHelper", "AboutChecking", "AboutCheckForUpdates", "UpdateAvailableMessage", "BannerDownloadButton", "BannerViewRelease", "UpdateDownloading", "UpdateReady", "BannerInstallRestart", "AboutUpToDate"] },
];

export function searchSettings(query: string, translate: (key: LocaleKey) => string, providerNames: string[] = []) {
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  return SETTINGS_SEARCH_TARGETS.filter(target => {
    const text = [translate(target.label), ...target.keywords.map(translate),
      ...(target.tab === "providers" ? providerNames : [])].join(" ").toLocaleLowerCase();
    return words.every(word => text.includes(word));
  });
}
