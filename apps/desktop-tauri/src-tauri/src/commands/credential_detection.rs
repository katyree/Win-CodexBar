use super::*;

// ── Credential detection ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCliStatus {
    pub signed_in: bool,
    pub credentials_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VertexAiStatus {
    pub has_credentials: bool,
    pub credentials_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JetbrainsIde {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub detected: bool,
    pub is_custom: bool,
    pub selected: bool,
    pub source: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroStatus {
    pub available: bool,
    pub hint: Option<String>,
}

fn gemini_cli_credentials_path() -> Option<std::path::PathBuf> {
    codexbar::host::session::gemini_cli_credentials_path()
}

fn vertexai_credentials_path_raw() -> Option<std::path::PathBuf> {
    codexbar::host::session::vertexai_credentials_path()
}

fn jetbrains_detected_ide_paths() -> Vec<std::path::PathBuf> {
    codexbar::host::session::jetbrains_detected_ide_paths()
}

#[tauri::command]
pub fn get_gemini_cli_signed_in() -> Result<GeminiCliStatus, String> {
    let path = gemini_cli_credentials_path();
    let signed_in = path.as_ref().map(|p| p.exists()).unwrap_or(false);
    Ok(GeminiCliStatus {
        signed_in,
        credentials_path: path.map(|p| p.to_string_lossy().into_owned()),
    })
}

#[tauri::command]
pub fn get_vertexai_status() -> Result<VertexAiStatus, String> {
    let path = vertexai_credentials_path_raw();
    let has = path.as_ref().map(|p| p.exists()).unwrap_or(false);
    Ok(VertexAiStatus {
        has_credentials: has,
        credentials_path: path.map(|p| p.to_string_lossy().into_owned()),
    })
}

#[tauri::command]
pub fn list_jetbrains_detected_ides() -> Result<Vec<JetbrainsIde>, String> {
    let settings = Settings::load();
    let override_path = settings.jetbrains_ide_base_path().to_string();
    let central = codexbar::providers::jetbrains::find_central_cli();

    let mut entries: Vec<JetbrainsIde> = jetbrains_detected_ide_paths()
        .into_iter()
        .map(|p| {
            let display = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string());
            JetbrainsIde {
                id: display.to_lowercase(),
                display_name: display,
                path: p.to_string_lossy().into_owned(),
                detected: true,
                is_custom: false,
                selected: false,
                source: "local",
            }
        })
        .collect();

    if !override_path.is_empty() {
        entries.retain(|entry| entry.path != override_path);
        let path_buf = std::path::PathBuf::from(&override_path);
        let display = path_buf
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| override_path.clone());
        entries.push(JetbrainsIde {
            id: format!("override::{display}").to_lowercase(),
            display_name: display,
            path: override_path.clone(),
            detected: codexbar::providers::jetbrains::discovery::quota_file(&path_buf).is_some(),
            is_custom: true,
            selected: false,
            source: "local",
        });
    }

    if let Some(cli) = central {
        entries.insert(
            0,
            JetbrainsIde {
                id: "central-cli".into(),
                display_name: "JetBrains Central CLI".into(),
                path: cli.to_string_lossy().into_owned(),
                detected: true,
                is_custom: false,
                selected: true,
                source: "cli",
            },
        );
    } else if let Some(entry) = entries
        .iter_mut()
        .find(|entry| entry.detected && (override_path.is_empty() || entry.is_custom))
    {
        entry.selected = true;
    }

    Ok(entries)
}

#[tauri::command]
pub fn set_jetbrains_ide_path(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    let pb = std::path::PathBuf::from(trimmed);
    if !trimmed.is_empty() && !pb.is_absolute() {
        return Err("JetBrains IDE path must be absolute".to_string());
    }
    if !trimmed.is_empty() && codexbar::providers::jetbrains::discovery::quota_file(&pb).is_none() {
        return Err("Select the IDE configuration folder containing options/AIAssistantQuotaManager2.xml, not the AI Assistant log folder. Clear the path to use automatic detection.".into());
    }
    let mut settings = Settings::load();
    settings.set_jetbrains_ide_base_path(pb.to_string_lossy().into_owned());
    settings.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_kiro_status() -> Result<KiroStatus, String> {
    if let Some(path) = codexbar::providers::kiro::find_kiro_cli() {
        Ok(KiroStatus {
            available: true,
            hint: Some(path.to_string_lossy().into_owned()),
        })
    } else {
        Ok(KiroStatus {
            available: false,
            hint: Some("kiro-cli: not found on PATH or known install locations".into()),
        })
    }
}
