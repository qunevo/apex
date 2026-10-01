//! Explicit allowlist of package folders. Loading a manifest never executes code.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub customization_root: PathBuf,
    pub default_customization: String,
    pub enabled_customizations: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct LoadedConfig {
    pub default: String,
    pub packages: BTreeMap<String, Package>,
}

impl LoadedConfig {
    pub fn select(&self, id: Option<&str>) -> Result<Package, String> {
        let id = id.unwrap_or(&self.default);
        self.packages
            .get(id)
            .cloned()
            .ok_or_else(|| format!("Customization is not enabled: {id}"))
    }

    pub fn check(&self, package: Option<&Package>) -> Result<(), String> {
        let package = package.ok_or("Scenario has no customization package; import it explicitly into this configured server")?;
        if self.packages.get(&package.id) != Some(package) {
            return Err(format!(
                "Customization version is not enabled: {}@{}",
                package.id, package.version
            ));
        }
        Ok(())
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        && !matches!(
            value.to_ascii_uppercase().as_str(),
            "CON" | "PRN" | "AUX" | "NUL"
        )
        && !(value.len() == 4
            && (value.starts_with("com") || value.starts_with("lpt"))
            && matches!(value.as_bytes()[3], b'1'..=b'9'))
}

impl Config {
    pub fn load(path: &Path) -> Result<LoadedConfig, Box<dyn std::error::Error>> {
        let path = path.canonicalize()?;
        let config: Self = serde_json::from_slice(&fs::read(&path)?)?;
        let root = path
            .parent()
            .unwrap()
            .join(&config.customization_root)
            .canonicalize()?;
        let mut packages = BTreeMap::new();
        for id in &config.enabled_customizations {
            if !identifier(id) || packages.contains_key(id) {
                return Err(format!("Invalid or duplicate customization folder: {id}").into());
            }
            let folder = root.join(id).canonicalize()?;
            let manifest = folder.join("package.json").canonicalize()?;
            if !folder.starts_with(&root) || !manifest.starts_with(&folder) {
                return Err("Customization manifest must remain inside its package root".into());
            }
            let package: Package = serde_json::from_slice(&fs::read(manifest)?)?;
            if package.id != *id || !identifier(&package.version) {
                return Err(format!("Invalid customization identity/version: {id}").into());
            }
            packages.insert(id.clone(), package);
        }
        if !packages.contains_key(&config.default_customization) {
            return Err("The default customization must be enabled".into());
        }
        Ok(LoadedConfig {
            default: config.default_customization,
            packages,
        })
    }
}
