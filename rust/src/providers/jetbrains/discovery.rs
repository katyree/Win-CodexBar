use std::path::{Path, PathBuf};

use crate::core::ProviderError;

const QUOTA_FILES: &[&str] = &[
    "AIAssistantQuotaManager2.xml",
    "ai-assistant.xml",
    "aiAssistant.xml",
    "ai.xml",
];

/// Accept an IDE configuration directory or its options directory, not IDE logs.
pub fn quota_file(ide_path: &Path) -> Option<PathBuf> {
    let options = if ide_path.file_name().is_some_and(|name| name == "options") {
        ide_path.to_path_buf()
    } else {
        ide_path.join("options")
    };
    QUOTA_FILES
        .iter()
        .map(|name| options.join(name))
        .find(|path| path.is_file())
}

pub fn detected_ide_paths() -> Vec<PathBuf> {
    dirs::config_dir()
        .map(|root| detected_ide_paths_in(&root))
        .unwrap_or_default()
}

fn detected_ide_paths_in(config_root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for vendor in ["JetBrains", "Google"] {
        if let Ok(entries) = std::fs::read_dir(config_root.join(vendor)) {
            for entry in entries.flatten() {
                let ide_path = entry.path();
                if let Some(quota) = quota_file(&ide_path) {
                    let modified = std::fs::metadata(quota).and_then(|m| m.modified()).ok();
                    candidates.push((modified, ide_path));
                }
            }
        }
    }
    // The most recently saved quota wins, not the first alphabetic IDE/version.
    candidates.sort_by(|a, b| b.cmp(a));
    candidates.into_iter().map(|(_, path)| path).collect()
}

pub fn select_quota_file(
    custom_path: &str,
    detected: &[PathBuf],
) -> Result<PathBuf, ProviderError> {
    if !custom_path.trim().is_empty() {
        return quota_file(Path::new(custom_path.trim())).ok_or_else(|| {
            ProviderError::Other(
                "No JetBrains AI quota file in the custom path. Select the IDE configuration folder containing options/AIAssistantQuotaManager2.xml, or clear the path for automatic detection."
                    .into(),
            )
        });
    }
    detected
        .iter()
        .find_map(|path| quota_file(path))
        .ok_or_else(|| {
            ProviderError::NotInstalled(
            "JetBrains AI quota not found. Open AI Assistant in your IDE and refresh its usage."
                .into(),
        )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn write_quota(root: &Path, relative: &str, seconds: u64) -> PathBuf {
        let ide = root.join(relative);
        std::fs::create_dir_all(ide.join("options")).unwrap();
        let file = std::fs::File::create(ide.join("options/AIAssistantQuotaManager2.xml")).unwrap();
        file.set_modified(UNIX_EPOCH + Duration::from_secs(seconds))
            .unwrap();
        ide
    }

    #[test]
    fn discovers_versioned_ides_and_selects_the_newest_quota() {
        let root = tempfile::tempdir().unwrap();
        let older = write_quota(root.path(), "JetBrains/IntelliJIdea2025.3", 100);
        let newest = write_quota(root.path(), "JetBrains/IntelliJIdea2026.2", 300);
        let android = write_quota(root.path(), "Google/AndroidStudio2026.1", 200);
        std::fs::create_dir_all(root.path().join("JetBrains/Air")).unwrap();
        std::fs::create_dir_all(root.path().join("JetBrains/PrivacyPolicy")).unwrap();
        let detected = detected_ide_paths_in(root.path());
        assert_eq!(detected, vec![newest.clone(), android, older]);
        assert_eq!(
            select_quota_file("", &detected).unwrap(),
            quota_file(&newest).unwrap()
        );
    }

    #[test]
    fn explicit_custom_path_wins_and_invalid_path_does_not_fall_back() {
        let root = tempfile::tempdir().unwrap();
        let automatic = write_quota(root.path(), "JetBrains/IntelliJIdea2026.2", 300);
        let custom = write_quota(root.path(), "CustomIDE", 100);
        let detected = vec![automatic];
        assert_eq!(
            select_quota_file(custom.to_str().unwrap(), &detected).unwrap(),
            quota_file(&custom).unwrap()
        );
        assert_eq!(quota_file(&custom.join("options")), quota_file(&custom));
        assert!(select_quota_file(root.path().to_str().unwrap(), &detected).is_err());
    }
}
