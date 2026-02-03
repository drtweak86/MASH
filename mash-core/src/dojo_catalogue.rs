//! Dojo Program Catalogue schema (InstallSpec).
//!
//! This is the Fedora-first, intent-only catalogue data model used to drive
//! Dojo program selection and provisioning decisions.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportedDistro {
    Fedora,
    Debian,
    Ubuntu,
    Arch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallMethod {
    /// Install using the system package manager (Fedora-first: `dnf`).
    Dnf,
    /// No automated install (placeholder for future methods).
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultTier {
    CoreDefault,
    Champion,
    Alternative,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Safe,
    Spicy,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PackageSpec {
    /// Fedora RPM package names.
    #[serde(default)]
    pub fedora: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallSpec {
    pub id: String,
    pub label: String,
    pub description: String,

    #[serde(default)]
    pub reason_why: Option<String>,

    pub install_method: InstallMethod,

    #[serde(default)]
    pub packages: PackageSpec,

    /// IDs of other specs that must be installed before this one.
    #[serde(default)]
    pub requires: Vec<String>,

    /// IDs of specs that cannot be selected together with this one.
    #[serde(default)]
    pub conflicts_with: Vec<String>,

    /// IDs of specs that are viable substitutes for this one.
    #[serde(default)]
    pub alternatives: Vec<String>,

    pub default_tier: DefaultTier,
    pub risk_level: RiskLevel,

    /// Fedora-first scope guard.
    pub supported_distros: Vec<SupportedDistro>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CategorySpec {
    pub id: String,
    pub label: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub programs: Vec<InstallSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DojoCatalogue {
    pub schema_version: u32,

    #[serde(default)]
    pub categories: Vec<CategorySpec>,
}

impl DojoCatalogue {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version == 0 {
            return Err(anyhow!("schema_version must be >= 1"));
        }

        let mut category_ids = HashSet::new();
        let mut specs_by_id: HashMap<&str, &InstallSpec> = HashMap::new();

        for category in &self.categories {
            if !category_ids.insert(category.id.as_str()) {
                return Err(anyhow!("duplicate category id: {}", category.id));
            }

            for spec in &category.programs {
                if specs_by_id.insert(spec.id.as_str(), spec).is_some() {
                    return Err(anyhow!("duplicate install spec id: {}", spec.id));
                }
            }
        }

        // Validate each spec's internal invariants and references.
        for spec in specs_by_id.values() {
            validate_spec(spec)?;
            validate_ref_list("requires", &spec.id, &spec.requires, &specs_by_id)?;
            validate_ref_list(
                "conflicts_with",
                &spec.id,
                &spec.conflicts_with,
                &specs_by_id,
            )?;
            validate_ref_list("alternatives", &spec.id, &spec.alternatives, &specs_by_id)?;

            let requires: HashSet<&str> = spec.requires.iter().map(|s| s.as_str()).collect();
            for conflict in &spec.conflicts_with {
                if requires.contains(conflict.as_str()) {
                    return Err(anyhow!(
                        "spec {} both requires and conflicts_with {}",
                        spec.id,
                        conflict
                    ));
                }
            }
        }

        // Basic conflict rule: conflicts are symmetric.
        for spec in specs_by_id.values() {
            for other_id in &spec.conflicts_with {
                let Some(other) = specs_by_id.get(other_id.as_str()) else {
                    continue; // already caught above
                };
                if !other.conflicts_with.iter().any(|id| id == &spec.id) {
                    return Err(anyhow!(
                        "conflict must be symmetric: {} conflicts_with {}, but {} does not list {}",
                        spec.id,
                        other_id,
                        other_id,
                        spec.id
                    ));
                }
            }
        }

        Ok(())
    }
}

fn validate_ref_list<'a>(
    field: &str,
    self_id: &str,
    refs: &[String],
    specs_by_id: &HashMap<&'a str, &'a InstallSpec>,
) -> Result<()> {
    let mut seen = HashSet::new();
    for id in refs {
        if id == self_id {
            return Err(anyhow!("spec {} lists itself in {}", self_id, field));
        }
        if !seen.insert(id.as_str()) {
            return Err(anyhow!(
                "spec {} has duplicate entry {} in {}",
                self_id,
                id,
                field
            ));
        }
        if !specs_by_id.contains_key(id.as_str()) {
            return Err(anyhow!(
                "spec {} references unknown id {} in {}",
                self_id,
                id,
                field
            ));
        }
    }
    Ok(())
}

fn validate_spec_fedora_first(spec: &InstallSpec) -> Result<()> {
    if spec.supported_distros.is_empty() {
        return Err(anyhow!("spec {} must list supported_distros", spec.id));
    }

    match spec.install_method {
        InstallMethod::Dnf => {
            if !spec.supported_distros.contains(&SupportedDistro::Fedora) {
                return Err(anyhow!(
                    "spec {} uses install_method=dnf but supported_distros does not include fedora",
                    spec.id
                ));
            }
            if spec.packages.fedora.is_empty() {
                return Err(anyhow!(
                    "spec {} uses install_method=dnf but packages.fedora is empty",
                    spec.id
                ));
            }
        }
        InstallMethod::Manual => {}
    }

    Ok(())
}

fn validate_spec(spec: &InstallSpec) -> Result<()> {
    validate_spec_fedora_first(spec)
}

impl InstallSpec {
    pub fn supports_distro(&self, distro: SupportedDistro) -> bool {
        self.supported_distros.contains(&distro)
    }
}

impl CategorySpec {
    pub fn filtered_programs(&self, distro: SupportedDistro) -> Vec<&InstallSpec> {
        self.programs
            .iter()
            .filter(|spec| spec.supports_distro(distro))
            .collect()
    }

    /// Visible program list for a given distro.
    ///
    /// - Default view: CoreDefault + Champion + Alternative (capped at 5).
    /// - Expanded view: top 5 programs (in catalogue order) after distro filtering.
    pub fn visible_programs(&self, distro: SupportedDistro, expanded: bool) -> Vec<&InstallSpec> {
        let filtered = self.filtered_programs(distro);
        if expanded {
            return filtered.into_iter().take(5).collect();
        }

        let mut visible = Vec::new();
        for spec in &filtered {
            if spec.default_tier == DefaultTier::CoreDefault {
                visible.push(*spec);
            }
        }
        if let Some(champion) = filtered
            .iter()
            .find(|spec| spec.default_tier == DefaultTier::Champion)
        {
            if !visible.iter().any(|s| s.id == champion.id) {
                visible.push(*champion);
            }
        }
        if let Some(alt) = filtered
            .iter()
            .find(|spec| spec.default_tier == DefaultTier::Alternative)
        {
            if !visible.iter().any(|s| s.id == alt.id) {
                visible.push(*alt);
            }
        }

        visible.truncate(5);
        visible
    }
}

pub fn parse_catalogue_toml(toml_str: &str) -> Result<DojoCatalogue> {
    let catalogue: DojoCatalogue = toml::from_str(toml_str).context("failed to parse catalogue")?;
    catalogue.validate()?;
    Ok(catalogue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_validates_minimal() {
        let doc = r#"
schema_version = 1

[[categories]]
id = "shell"
label = "Shell"

  [[categories.programs]]
  id = "zsh"
  label = "Zsh"
  description = "Z shell"
  install_method = "dnf"
  packages = { fedora = ["zsh"] }
  requires = []
  conflicts_with = []
  alternatives = []
  default_tier = "core_default"
  risk_level = "safe"
  supported_distros = ["fedora"]

  [[categories.programs]]
  id = "fish"
  label = "Fish"
  description = "Friendly interactive shell"
  install_method = "dnf"
  packages = { fedora = ["fish"] }
  requires = []
  conflicts_with = ["zsh"]
  alternatives = []
  default_tier = "alternative"
  risk_level = "safe"
  supported_distros = ["fedora"]

"#;

        // Make the conflict symmetric.
        let doc = doc.replace("conflicts_with = []", "conflicts_with = [\"fish\"]");
        let parsed = parse_catalogue_toml(&doc).unwrap();
        assert_eq!(parsed.schema_version, 1);
        assert_eq!(parsed.categories.len(), 1);
        assert_eq!(parsed.categories[0].programs.len(), 2);
    }

    #[test]
    fn rejects_unknown_reference() {
        let doc = r#"
schema_version = 1

[[categories]]
id = "shell"
label = "Shell"

  [[categories.programs]]
  id = "zsh"
  label = "Zsh"
  description = "Z shell"
  install_method = "dnf"
  packages = { fedora = ["zsh"] }
  conflicts_with = ["does_not_exist"]
  default_tier = "core_default"
  risk_level = "safe"
  supported_distros = ["fedora"]
"#;
        let err = parse_catalogue_toml(doc).unwrap_err().to_string();
        assert!(err.contains("unknown id"));
    }

    #[test]
    fn rejects_asymmetric_conflict() {
        let doc = r#"
schema_version = 1

[[categories]]
id = "shell"
label = "Shell"

  [[categories.programs]]
  id = "zsh"
  label = "Zsh"
  description = "Z shell"
  install_method = "dnf"
  packages = { fedora = ["zsh"] }
  conflicts_with = ["fish"]
  default_tier = "core_default"
  risk_level = "safe"
  supported_distros = ["fedora"]

  [[categories.programs]]
  id = "fish"
  label = "Fish"
  description = "Friendly interactive shell"
  install_method = "dnf"
  packages = { fedora = ["fish"] }
  conflicts_with = []
  default_tier = "alternative"
  risk_level = "safe"
  supported_distros = ["fedora"]
"#;
        let err = parse_catalogue_toml(doc).unwrap_err().to_string();
        assert!(err.contains("conflict must be symmetric"));
    }

    #[test]
    fn rejects_duplicate_spec_ids() {
        let doc = r#"
schema_version = 1

[[categories]]
id = "shell"
label = "Shell"

  [[categories.programs]]
  id = "zsh"
  label = "Zsh"
  description = "Z shell"
  install_method = "dnf"
  packages = { fedora = ["zsh"] }
  conflicts_with = []
  default_tier = "core_default"
  risk_level = "safe"
  supported_distros = ["fedora"]

  [[categories.programs]]
  id = "zsh"
  label = "Zsh 2"
  description = "Duplicate id"
  install_method = "dnf"
  packages = { fedora = ["zsh"] }
  conflicts_with = []
  default_tier = "alternative"
  risk_level = "safe"
  supported_distros = ["fedora"]
"#;
        let err = parse_catalogue_toml(doc).unwrap_err().to_string();
        assert!(err.contains("duplicate install spec id"));
    }

    #[test]
    fn filters_programs_by_distro_and_respects_default_view() {
        let doc = r#"
schema_version = 1

[[categories]]
id = "shell"
label = "Shell"

  [[categories.programs]]
  id = "core"
  label = "Core tool"
  description = "Core tool"
  install_method = "dnf"
  packages = { fedora = ["coreutils"] }
  default_tier = "core_default"
  risk_level = "safe"
  supported_distros = ["fedora", "debian"]

  [[categories.programs]]
  id = "champ"
  label = "Champion"
  description = "Champion"
  install_method = "dnf"
  packages = { fedora = ["zsh"] }
  default_tier = "champion"
  risk_level = "safe"
  supported_distros = ["fedora"]

  [[categories.programs]]
  id = "alt"
  label = "Alternative"
  description = "Alternative"
  install_method = "dnf"
  packages = { fedora = ["fish"] }
  default_tier = "alternative"
  risk_level = "safe"
  supported_distros = ["fedora"]

  [[categories.programs]]
  id = "debian_only"
  label = "Debian only"
  description = "Debian only"
  install_method = "manual"
  default_tier = "optional"
  risk_level = "safe"
  supported_distros = ["debian"]
"#;
        let catalogue = parse_catalogue_toml(doc).unwrap();
        let cat = &catalogue.categories[0];

        let visible = cat.visible_programs(SupportedDistro::Fedora, false);
        let ids: Vec<&str> = visible.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["core", "champ", "alt"]);

        let expanded = cat.visible_programs(SupportedDistro::Fedora, true);
        let expanded_ids: Vec<&str> = expanded.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(expanded_ids, vec!["core", "champ", "alt"]);
    }
}
