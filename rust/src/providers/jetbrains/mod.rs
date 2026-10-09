//! JetBrains AI provider implementation
//!
//! Fetches usage data from JetBrains IDE local configuration
//! JetBrains AI Assistant stores quota info in XML configuration files

mod central;
pub mod discovery;
mod quota;

pub use central::find_central_cli;

use async_trait::async_trait;

use crate::core::{
    FetchContext, Provider, ProviderError, ProviderFetchResult, ProviderId, ProviderMetadata,
    SourceMode, UsageSnapshot,
};

/// JetBrains AI provider
pub struct JetBrainsProvider {
    metadata: ProviderMetadata,
}

impl JetBrainsProvider {
    pub fn new() -> Self {
        Self {
            metadata: ProviderMetadata {
                id: ProviderId::JetBrains,
                display_name: "JetBrains AI",
                session_label: "Credits",
                weekly_label: "Monthly",
                supports_opus: false,
                supports_credits: true,
                default_enabled: false,
                is_primary: false,
                dashboard_url: Some("https://www.jetbrains.com/ai/"),
                status_page_url: None,
                tertiary_label_key: None,
            },
        }
    }

    /// Read the IDE cache only when the live Central CLI is not installed.
    async fn read_local_config(&self) -> Result<UsageSnapshot, ProviderError> {
        let settings = crate::settings::Settings::load();
        let config_file = discovery::select_quota_file(
            settings.jetbrains_ide_base_path(),
            &discovery::detected_ide_paths(),
        )?;
        let content = tokio::fs::read_to_string(&config_file)
            .await
            .map_err(|_| ProviderError::Other("Failed to read JetBrains AI quota file".into()))?;
        let mut usage = self.parse_xml_config(&content)?;
        // Refreshing CodexBar does not refresh the IDE's saved quota.
        usage.updated_at = std::fs::metadata(&config_file)
            .and_then(|metadata| metadata.modified())
            .map(chrono::DateTime::<chrono::Utc>::from)
            .map_err(|_| {
                ProviderError::Other("Failed to read JetBrains AI quota timestamp".into())
            })?;
        Ok(usage)
    }

    fn parse_xml_config(&self, content: &str) -> Result<UsageSnapshot, ProviderError> {
        quota::parse(content)
    }
}

impl Default for JetBrainsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for JetBrainsProvider {
    fn id(&self) -> ProviderId {
        ProviderId::JetBrains
    }

    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn fetch_usage(&self, ctx: &FetchContext) -> Result<ProviderFetchResult, ProviderError> {
        tracing::debug!("Fetching JetBrains AI usage");

        match ctx.source_mode {
            SourceMode::Auto | SourceMode::Cli => {
                if let Some(cli) = find_central_cli() {
                    let usage = central::fetch(&cli, ctx.web_timeout).await?;
                    return Ok(ProviderFetchResult::new(usage, "cli"));
                }
                let usage = self.read_local_config().await?;
                Ok(ProviderFetchResult::new(usage, "local"))
            }
            SourceMode::Web | SourceMode::OAuth => {
                // JetBrains AI doesn't have web API access
                Err(ProviderError::UnsupportedSource(ctx.source_mode))
            }
        }
    }

    fn available_sources(&self) -> Vec<SourceMode> {
        vec![SourceMode::Auto, SourceMode::Cli]
    }

    fn supports_web(&self) -> bool {
        false
    }

    fn supports_cli(&self) -> bool {
        true
    }
    /// JetBrains' local IDE probe raises `NotInstalled` when the AI
    /// Assistant plugin is not found in any IDE configuration — an
    /// installation gap, not a credential problem — so it surfaces as an
    /// offline local runtime (matching the pre-backend classifier's
    /// treatment of plugin-presence failures). This is the provider's only
    /// `NotInstalled` producer, so the variant maps wholesale.
    fn error_state_kind(&self, error: &ProviderError) -> crate::core::ProviderStateKind {
        match error {
            ProviderError::NotInstalled(_) => crate::core::ProviderStateKind::LocalRuntimeOffline,
            _ => error.state_kind(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_quota_info_and_next_refill_from_ide_xml() {
        let xml = r#"<application><component name="AIAssistantQuotaManager2">
          <option name="quotaInfo" value="{&quot;type&quot;:&quot;Available&quot;,&quot;current&quot;:&quot;250&quot;,&quot;maximum&quot;:&quot;1000&quot;,&quot;until&quot;:&quot;2027-08-31T23:59:59Z&quot;}" />
          <option name="nextRefill" value="{&quot;type&quot;:&quot;Known&quot;,&quot;next&quot;:&quot;2026-10-31T23:59:59Z&quot;}" />
        </component></application>"#;
        let usage = JetBrainsProvider::new().parse_xml_config(xml).unwrap();
        assert_eq!(usage.primary.used_percent, 25.0);
        assert_eq!(
            usage.primary.resets_at.unwrap().to_rfc3339(),
            "2026-10-31T23:59:59+00:00"
        );
    }

    #[test]
    fn missing_quota_is_an_error_instead_of_zero_usage() {
        assert!(
            JetBrainsProvider::new()
                .parse_xml_config(r#"<application><component name="AiAssistant" /></application>"#)
                .is_err()
        );
    }

    #[test]
    fn reads_numeric_quota_json_with_xml_newlines_and_legacy_credit_options() {
        let provider = JetBrainsProvider::new();
        let current = provider.parse_xml_config(
            r#"<option value='{&#10;"type":"Available","current":30,"maximum":200}' name='quotaInfo' />"#,
        ).unwrap();
        assert_eq!(current.primary.used_percent, 15.0);
        assert!(current.primary.resets_at.is_none());
        let legacy = provider.parse_xml_config(
            r#"<option name="usedCredits" value="30"/><option name="creditLimit" value="200"/>"#,
        ).unwrap();
        assert_eq!(legacy.primary.used_percent, 15.0);
    }

    #[test]
    fn plugin_presence_maps_to_local_runtime_offline() {
        assert_eq!(
            JetBrainsProvider::new().error_state_kind(&ProviderError::NotInstalled(
                "JetBrains AI Assistant not found. Install from JetBrains IDE Marketplace."
                    .to_string(),
            )),
            crate::core::ProviderStateKind::LocalRuntimeOffline
        );
    }
}
