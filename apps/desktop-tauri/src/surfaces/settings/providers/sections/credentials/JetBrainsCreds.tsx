import { useEffect, useState } from "react";
import type { LocaleKey } from "../../../../../i18n/keys";
import type { JetbrainsIde } from "../../../../../types/bridge";
import {
  listJetbrainsDetectedIdes,
  openPath,
  refreshProviders,
  setJetbrainsIdePath,
} from "../../../../../lib/tauri";

interface Props {
  t: (key: LocaleKey) => string;
}

/** Live Central CLI source and saved IDE quota fallback. */
export function JetBrainsCreds({ t }: Props) {
  const [ides, setIdes] = useState<JetbrainsIde[]>([]);
  const [customPath, setCustomPath] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = async () => {
    try {
      const next = await listJetbrainsDetectedIdes();
      setIdes(next);
      const override = next.find(ide => ide.isCustom);
      setCustomPath(override?.path ?? "");
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    void reload();
  }, []);

  const primaryDetected = ides.find(ide => ide.selected);
  const statusLabel = primaryDetected
    ? t("CredsStatusDetected")
    : t("CredsStatusNotDetected");

  const handleOpenFolder = (path: string) => {
    void openPath(path).catch((e) => setError(String(e)));
  };

  const handleSavePath = async () => {
    setBusy(true);
    try {
      await setJetbrainsIdePath(customPath.trim());
      await refreshProviders();
      await reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleRefresh = async () => {
    setBusy(true);
    try {
      await refreshProviders();
      await reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="provider-detail-section">
      <h4>{t("CredentialsSectionTitle")}</h4>
      <dl className="provider-detail-grid">
        <div style={{ display: "contents" }}>
          <dt>{t("CredsJetBrainsLabel")}</dt>
          <dd>{statusLabel}</dd>
        </div>
      </dl>

      {ides.length > 0 && (
        <ul className="provider-detail-list">
          {ides.map(ide => (
            <li key={ide.id} className="provider-detail-list__row">
              <div className="provider-detail-list__main">
                <div>{ide.displayName}</div>
                <div className="provider-detail-grid__mono">{ide.path}</div>
              </div>
              <div className="provider-detail-list__meta">
                {ide.detected
                  ? t("CredsStatusDetected")
                  : t("CredsStatusNotDetected")}
              </div>
              {ide.source === "local" && <button
                type="button"
                className="btn btn--ghost"
                onClick={() => handleOpenFolder(ide.path)}
              >
                {t("CredsOpenFolderAction")}
              </button>}
            </li>
          ))}
        </ul>
      )}

      <div className="provider-detail-helper">
        {primaryDetected
          ? primaryDetected.source === "cli"
            ? `${primaryDetected.displayName}: ${primaryDetected.path}`
            : `${t("CredsJetBrainsHelperDetectedPrefix")} ${primaryDetected.path}.`
          : t("CredsJetBrainsHelperMissing")}
      </div>

      <label className="provider-detail-field">
        <span className="provider-detail-field__label">
          {t("CredsJetBrainsCustomPathLabel")}
        </span>
        <input
          type="text"
          className="provider-detail-field__input"
          value={customPath}
          placeholder={t("CredsJetBrainsCustomPathPlaceholder")}
          onChange={e => setCustomPath(e.target.value)}
        />
      </label>

      <div className="provider-detail-actions">
        <button
          type="button"
          className="btn btn--ghost"
          onClick={handleSavePath}
          disabled={busy}
        >
          {t("CredsSavePathAction")}
        </button>
        <button
          type="button"
          className="btn btn--ghost"
          onClick={handleRefresh}
          disabled={busy}
        >
          {t("CredsRefreshDetectionAction")}
        </button>
      </div>

      {error && <div className="provider-detail-error">{error}</div>}
    </section>
  );
}
