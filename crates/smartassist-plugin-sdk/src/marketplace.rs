//! Skills Marketplace (ClawHub) client.
//!
//! Provides registry fetch, skill metadata resolution, and install/uninstall
//! operations for the SmartAssist plugin ecosystem.

use crate::{PluginError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Skill manifest from the marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Unique skill ID.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Skill version.
    pub version: String,
    /// Short description.
    pub description: String,
    /// Author information.
    pub author: Option<String>,
    /// Download URL for the skill package.
    pub download_url: String,
    /// SHA256 checksum of the package.
    pub checksum: Option<String>,
    /// Minimum SmartAssist version required.
    pub min_smartassist_version: Option<String>,
    /// Skill tags/categories.
    pub tags: Vec<String>,
    /// Dependencies on other skills.
    pub dependencies: Vec<String>,
    /// When the skill was published.
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Marketplace catalog response.
#[derive(Debug, Clone, Deserialize)]
pub struct CatalogResponse {
    /// Available skills.
    pub skills: Vec<SkillManifest>,
    /// Total count.
    pub total: usize,
    /// Catalog version.
    pub version: String,
}

/// Install status of a skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStatus {
    /// Skill is not installed.
    NotInstalled,
    /// Download in progress.
    Downloading,
    /// Installing.
    Installing,
    /// Installed and ready.
    Installed,
    /// Install failed.
    Failed,
    /// Being uninstalled.
    Uninstalling,
}

/// Installed skill record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSkill {
    /// Skill manifest.
    pub manifest: SkillManifest,
    /// Current install status.
    pub status: InstallStatus,
    /// Installation path.
    pub install_path: std::path::PathBuf,
    /// When installed.
    pub installed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Install error (if failed).
    pub error: Option<String>,
}

/// Marketplace client for ClawHub.
pub struct MarketplaceClient {
    /// Base URL of the marketplace API.
    base_url: String,
    /// HTTP client.
    client: reqwest::Client,
}

impl MarketplaceClient {
    /// Create a new marketplace client.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Fetch the skill catalog.
    pub async fn fetch_catalog(&self,
        query: Option<&str>,
    ) -> Result<CatalogResponse> {
        let mut url = format!("{}/api/v1/skills", self.base_url);
        if let Some(q) = query {
            url.push_str(&format!("?q={}", urlencoding::encode(q)));
        }

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PluginError::runtime(format!("Failed to fetch catalog: {}", e)))?;

        if !response.status().is_success() {
            return Err(PluginError::runtime(format!(
                "Catalog fetch failed: HTTP {}",
                response.status()
            )));
        }

        let catalog: CatalogResponse = response
            .json()
            .await
            .map_err(|e| PluginError::runtime(format!("Failed to parse catalog: {}", e)))?;

        Ok(catalog)
    }

    /// Fetch a single skill manifest.
    pub async fn fetch_skill(&self, skill_id: &str) -> Result<Option<SkillManifest>> {
        let url = format!("{}/api/v1/skills/{}", self.base_url, skill_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PluginError::runtime(format!("Failed to fetch skill: {}", e)))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(PluginError::runtime(format!(
                "Skill fetch failed: HTTP {}",
                response.status()
            )));
        }

        let manifest: SkillManifest = response
            .json()
            .await
            .map_err(|e| PluginError::runtime(format!("Failed to parse skill: {}", e)))?;

        Ok(Some(manifest))
    }
}

/// Skill installer that manages local skill packages.
pub struct SkillInstaller {
    /// Installation directory.
    install_dir: std::path::PathBuf,
    /// HTTP client for downloads.
    client: reqwest::Client,
    /// Installed skills index.
    index: HashMap<String, InstalledSkill>,
}

impl SkillInstaller {
    /// Create a new skill installer.
    pub fn new(install_dir: impl AsRef<Path>) -> Self {
        Self {
            install_dir: install_dir.as_ref().to_path_buf(),
            client: reqwest::Client::new(),
            index: HashMap::new(),
        }
    }

    /// Install a skill from its manifest.
    pub async fn install(&mut self,
        manifest: &SkillManifest,
    ) -> Result<InstalledSkill> {
        let skill_dir = self.install_dir.join(&manifest.id);
        std::fs::create_dir_all(&skill_dir).map_err(|e| {
            PluginError::runtime(format!("Failed to create skill dir: {}", e))
        })?;

        let mut skill = InstalledSkill {
            manifest: manifest.clone(),
            status: InstallStatus::Downloading,
            install_path: skill_dir.clone(),
            installed_at: None,
            error: None,
        };

        // Download the skill package
        let response = self
            .client
            .get(&manifest.download_url)
            .send()
            .await
            .map_err(|e| {
                skill.status = InstallStatus::Failed;
                skill.error = Some(e.to_string());
                PluginError::runtime(format!("Download failed: {}", e))
            })?;

        if !response.status().is_success() {
            skill.status = InstallStatus::Failed;
            skill.error = Some(format!("HTTP {}", response.status()));
            return Err(PluginError::runtime(format!(
                "Download failed: HTTP {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            skill.status = InstallStatus::Failed;
            skill.error = Some(e.to_string());
            PluginError::runtime(format!("Download failed: {}", e))
        })?;

        // Write the package to disk
        let package_path = skill_dir.join("package.zip");
        std::fs::write(&package_path, &bytes).map_err(|e| {
            skill.status = InstallStatus::Failed;
            skill.error = Some(e.to_string());
            PluginError::runtime(format!("Failed to write package: {}", e))
        })?;

        // Write manifest
        let manifest_path = skill_dir.join("manifest.json");
        let manifest_json = serde_json::to_string_pretty(manifest).map_err(|e| {
            PluginError::runtime(format!("Failed to serialize manifest: {}", e))
        })?;
        std::fs::write(&manifest_path, manifest_json).map_err(|e| {
            PluginError::runtime(format!("Failed to write manifest: {}", e))
        })?;

        skill.status = InstallStatus::Installed;
        skill.installed_at = Some(chrono::Utc::now());

        self.index.insert(manifest.id.clone(), skill.clone());
        Ok(skill)
    }

    /// Uninstall a skill by ID.
    pub fn uninstall(&mut self, skill_id: &str) -> Result<bool> {
        let skill_dir = self.install_dir.join(skill_id);
        if skill_dir.exists() {
            std::fs::remove_dir_all(&skill_dir).map_err(|e| {
                PluginError::runtime(format!("Failed to remove skill dir: {}", e))
            })?;
            self.index.remove(skill_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List installed skills.
    pub fn list_installed(&self) -> Vec<&InstalledSkill> {
        self.index.values().collect()
    }

    /// Get an installed skill.
    pub fn get(&self, skill_id: &str) -> Option<&InstalledSkill> {
        self.index.get(skill_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_skill_manifest_serialization() {
        let manifest = SkillManifest {
            id: "test-skill".to_string(),
            name: "Test Skill".to_string(),
            version: "1.0.0".to_string(),
            description: "A test skill".to_string(),
            author: Some("Test Author".to_string()),
            download_url: "https://example.com/skill.zip".to_string(),
            checksum: Some("sha256:abc123".to_string()),
            min_smartassist_version: Some("0.1.0".to_string()),
            tags: vec!["test".to_string()],
            dependencies: vec![],
            published_at: Some(chrono::Utc::now()),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        assert!(json.contains("test-skill"));

        let parsed: SkillManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-skill");
    }

    #[test]
    fn test_skill_installer() {
        let dir = TempDir::new().unwrap();
        let installer = SkillInstaller::new(dir.path());
        assert!(installer.list_installed().is_empty());
    }

    #[tokio::test]
    async fn test_install_and_uninstall() {
        let dir = TempDir::new().unwrap();
        let mut installer = SkillInstaller::new(dir.path());

        let manifest = SkillManifest {
            id: "my-skill".to_string(),
            name: "My Skill".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            author: None,
            download_url: "https://httpbin.org/bytes/1024".to_string(),
            checksum: None,
            min_smartassist_version: None,
            tags: vec![],
            dependencies: vec![],
            published_at: None,
        };

        // Note: this test requires network access; skip if offline
        if let Ok(skill) = installer.install(&manifest).await {
            assert_eq!(skill.status, InstallStatus::Installed);
            assert!(skill.install_path.exists());

            let uninstalled = installer.uninstall("my-skill").unwrap();
            assert!(uninstalled);
            assert!(!skill.install_path.exists());
        }
    }
}
