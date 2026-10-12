import type { CSSProperties } from "react";
import type { ProviderUsageSnapshot } from "../types/bridge";
import { useLocale } from "../hooks/useLocale";
import { hideOpenAiApiProjectId } from "../lib/openAiApiIdentity";
import { providerColor } from "../lib/providerPalette";
import { isUsageItemVisible } from "../lib/usageItemVisibility";
import { localizeWindowLabel } from "../lib/windowLabels";
import { ProviderIcon } from "./providers/ProviderIcon";
import { MetricRow } from "./MenuCardDetails";

export default function ProviderTile({ provider, resetTimeRelative, isRefreshing, showProviderIcon, hidePersonalInfo, onSelect }: {
  provider: ProviderUsageSnapshot;
  resetTimeRelative: boolean;
  isRefreshing: boolean;
  showProviderIcon: boolean;
  hidePersonalInfo: boolean;
  onSelect: () => void;
}) {
  const { t, language } = useLocale();
  const planName = hideOpenAiApiProjectId(provider.planName, provider.providerId === "openaiapi" && hidePersonalInfo);
  const windows = provider.providerId === "wayfinder" ? [] : [
    { id: "primary", snap: provider.primary, label: provider.primaryLabel },
    { id: "secondary", snap: provider.secondary, label: provider.secondaryLabel },
    { id: "model-specific", snap: provider.modelSpecific, label: t("DetailWindowModelSpecific") },
    { id: "tertiary", snap: provider.tertiary, label: provider.tertiaryLabel },
  ].filter(window => window.snap && isUsageItemVisible(provider.hiddenUsageItemIds, window.id));
  return (
    <button type="button" className="provider-tile" data-provider={provider.providerId}
      onClick={onSelect} aria-busy={isRefreshing}
      style={{ "--provider-accent": providerColor(provider.providerId) } as CSSProperties}>
      <span className="provider-tile__header">{showProviderIcon && <ProviderIcon providerId={provider.providerId} size={20} />}
        <strong>{provider.displayName}</strong><span aria-hidden>↗</span></span>
      {planName && provider.providerId !== "wayfinder" && <span className="provider-tile__plan">{planName}</span>}
      {provider.error ? <span className="provider-tile__error">{provider.error}</span> :
        <span className="provider-tile__metrics">{windows.map(window => window.snap && (
          <MetricRow key={window.id} title={localizeWindowLabel(window.label, t, language, window.snap.windowMinutes) || t("DetailWindowPrimary")}
            snap={window.snap} exhaustedLabel={t("PanelUsedSuffix")}
            display={{ largePercent: true, showAsUsed: false, showPace: false, resetTimeRelative }}
            expanded={false} onToggleExpanded={() => {}} />
        ))}</span>}
      <span className="provider-tile__link">{t("PanelUsageDetails")} →</span>
    </button>
  );
}
