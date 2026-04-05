use crate::skills::detect::DetectionRules;
use crate::skills::provider::{Provider, ProviderCatalogMetadata};
use crate::skills::suggest::{
    DetectionConfidence, SkillSuggestion, TechnologyDetection, TechnologyId,
};
use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Component, Path};
use tracing::warn;

const EMBEDDED_CATALOG_METADATA: &str = include_str!("catalog.v1.toml");
const EMBEDDED_SOURCE_NAME: &str = "embedded";
const EMBEDDED_METADATA_VERSION: &str = "v1";
const SUPPORTED_SCHEMA_VERSION: &str = "v1";
const LOCAL_EMBEDDED_SKILL_PREFIX: &str = "dallay/agents-skills/";
const DEFAULT_REASON_TEMPLATE: &str =
    "Recommended because {technology} was detected from {evidence}.";
const APPROVED_EMBEDDED_EXTERNAL_SKILL_IDS: &[&str] = &[
    "Kotlin/kotlin-agent-skills/kotlin-tooling-agp9-migration",
    "Kotlin/kotlin-agent-skills/kotlin-tooling-cocoapods-spm-migration",
    "addyosmani/web-quality-skills/accessibility",
    "addyosmani/web-quality-skills/seo",
    "affaan-m/everything-claude-code/java-coding-standards",
    "aj-geddes/useful-ai-prompts/nodejs-express-server",
    "angular/angular/PR Review",
    "angular/angular/adev-writing-guide",
    "angular/angular/reference-compiler-cli",
    "angular/angular/reference-core",
    "angular/angular/reference-signal-forms",
    "angular/skills/angular-developer",
    "antfu/skills/nuxt",
    "antfu/skills/vite",
    "antfu/skills/vitest",
    "antfu/skills/vue",
    "antfu/skills/vue-best-practices",
    "anthropics/skills/frontend-design",
    "apollographql/skills/graphql-schema",
    "astrolicious/agent-skills/astro",
    "avdlee/swiftui-agent-skill/swiftui-expert-skill",
    "awslabs/agent-plugins",
    "better-auth/skills/better-auth-best-practices",
    "better-auth/skills/email-and-password-best-practices",
    "better-auth/skills/organization-best-practices",
    "better-auth/skills/two-factor-authentication-best-practices",
    "bobmatnyc/claude-mpm-skills/drizzle-orm",
    "clerk/skills/clerk",
    "clerk/skills/clerk-custom-ui",
    "clerk/skills/clerk-nextjs-patterns",
    "clerk/skills/clerk-orgs",
    "clerk/skills/clerk-setup",
    "clerk/skills/clerk-testing",
    "clerk/skills/clerk-webhooks",
    "cloudflare/skills/agents-sdk",
    "cloudflare/skills/building-ai-agent-on-cloudflare",
    "cloudflare/skills/building-mcp-server-on-cloudflare",
    "cloudflare/skills/cloudflare",
    "cloudflare/skills/durable-objects",
    "cloudflare/skills/sandbox-sdk",
    "cloudflare/skills/web-perf",
    "cloudflare/skills/workers-best-practices",
    "cloudflare/skills/wrangler",
    "cloudflare/vinext/migrate-to-vinext",
    "currents-dev/playwright-best-practices-skill/playwright-best-practices",
    "delexw/claude-code-misc/oxlint",
    "denoland/skills/deno-deploy",
    "denoland/skills/deno-expert",
    "denoland/skills/deno-frontend",
    "denoland/skills/deno-guidance",
    "denoland/skills/deno-sandbox",
    "dotnet/skills",
    "ejirocodes/agent-skills/svelte5-best-practices",
    "expo/skills/building-native-ui",
    "expo/skills/expo-api-routes",
    "expo/skills/expo-cicd-workflows",
    "expo/skills/expo-deployment",
    "expo/skills/expo-dev-client",
    "expo/skills/expo-tailwind-setup",
    "expo/skills/native-data-fetching",
    "expo/skills/upgrading-expo",
    "expo/skills/use-dom",
    "flutter/skills",
    "github/awesome-copilot/java-docs",
    "github/awesome-copilot/java-springboot",
    "github/awesome-copilot/openapi-to-application-code",
    "giuseppe-trisciuoglio/developer-kit/tailwind-css-patterns",
    "googlecloudplatform/devrel-demos",
    "greensock/gsap-skills/gsap-core",
    "greensock/gsap-skills/gsap-frameworks",
    "greensock/gsap-skills/gsap-performance",
    "greensock/gsap-skills/gsap-plugins",
    "greensock/gsap-skills/gsap-react",
    "greensock/gsap-skills/gsap-scrolltrigger",
    "greensock/gsap-skills/gsap-timeline",
    "greensock/gsap-skills/gsap-utils",
    "hashicorp/agent-skills",
    "huggingface/skills",
    "hyf0/vue-skills/vue-best-practices",
    "hyf0/vue-skills/vue-debug-guides",
    "inferen-sh/skills/elevenlabs-music",
    "inferen-sh/skills/elevenlabs-tts",
    "kadajett/agent-nestjs-skills/nestjs-best-practices",
    "krutikJain/android-agent-skills/android-architecture-clean",
    "krutikJain/android-agent-skills/android-compose-foundations",
    "krutikJain/android-agent-skills/android-coroutines-flow",
    "krutikJain/android-agent-skills/android-di-hilt",
    "krutikJain/android-agent-skills/android-gradle-build-logic",
    "krutikJain/android-agent-skills/android-kotlin-core",
    "krutikJain/android-agent-skills/android-networking-retrofit-okhttp",
    "krutikJain/android-agent-skills/android-testing-unit",
    "langchain-ai/langchain-skills",
    "laravel/boost",
    "microsoft/github-copilot-for-azure/azure-ai",
    "microsoft/github-copilot-for-azure/azure-cost-optimization",
    "microsoft/github-copilot-for-azure/azure-deploy",
    "microsoft/github-copilot-for-azure/azure-diagnostics",
    "mindrally/skills/deno-typescript",
    "mongodb/agent-skills",
    "neondatabase/agent-skills/neon-postgres",
    "nodnarbnitram/claude-code-extensions/tauri-v2",
    "nrwl/nx-ai-agents-config",
    "openai/skills",
    "openai/skills/cloudflare-deploy",
    "prisma/skills/prisma-cli",
    "prisma/skills/prisma-client-api",
    "prisma/skills/prisma-database-setup",
    "prisma/skills/prisma-postgres",
    "pulumi/agent-skills",
    "pytorch/pytorch",
    "redis/agent-skills",
    "remix-run/agent-skills",
    "remotion-dev/skills/remotion-best-practices",
    "secondsky/claude-skills/tailwind-v4-shadcn",
    "shadcn/ui/shadcn",
    "sleekdotdesign/agent-skills/sleek-design-mobile-apps",
    "storybookjs/react-native",
    "stripe/ai/stripe-best-practices",
    "stripe/ai/upgrade-stripe",
    "supabase/agent-skills/supabase-postgres-best-practices",
    "sveltejs/ai-tools/svelte-code-writer",
    "vercel-labs/agent-skills/deploy-to-vercel",
    "vercel-labs/agent-skills/vercel-composition-patterns",
    "vercel-labs/agent-skills/vercel-react-best-practices",
    "vercel-labs/next-skills/next-best-practices",
    "vercel-labs/next-skills/next-cache-components",
    "vercel-labs/next-skills/next-upgrade",
    "vercel/ai/ai-sdk",
    "vercel/turborepo/turborepo",
    "vuejs-ai/skills/vue-pinia-best-practices",
    "wordpress/agent-skills/wordpress-router",
    "wordpress/agent-skills/wp-block-development",
    "wordpress/agent-skills/wp-block-themes",
    "wordpress/agent-skills/wp-performance",
    "wordpress/agent-skills/wp-plugin-development",
    "wordpress/agent-skills/wp-project-triage",
    "wordpress/agent-skills/wp-rest-api",
    "wordpress/agent-skills/wp-wpcli-and-ops",
    "yusukebe/hono-skill/hono",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSkillMetadata {
    pub provider_skill_id: String,
    pub skill_id: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSkillDefinition {
    pub provider_skill_id: String,
    pub local_skill_id: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogTechnologyEntry {
    pub id: TechnologyId,
    pub name: String,
    pub detect: Option<DetectionRules>,
    pub skills: Vec<String>,
    pub min_confidence: DetectionConfidence,
    pub reason_template: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogComboEntry {
    pub id: String,
    pub name: String,
    pub requires: Vec<TechnologyId>,
    pub skills: Vec<String>,
    pub enabled: bool,
    pub reason_template: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedSkillCatalog {
    source_name: String,
    metadata_version: String,
    skill_definitions: BTreeMap<String, CatalogSkillDefinition>,
    local_skills: BTreeMap<String, CatalogSkillMetadata>,
    technologies: BTreeMap<TechnologyId, CatalogTechnologyEntry>,
    combos: BTreeMap<String, CatalogComboEntry>,
}

impl ResolvedSkillCatalog {
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn metadata_version(&self) -> &str {
        &self.metadata_version
    }

    pub fn get_skill(&self, skill_id: &str) -> Option<&CatalogSkillMetadata> {
        self.local_skills.get(skill_id)
    }

    pub fn get_skill_definition(&self, provider_skill_id: &str) -> Option<&CatalogSkillDefinition> {
        self.skill_definitions.get(provider_skill_id)
    }

    pub fn get_technology(&self, technology: &TechnologyId) -> Option<&CatalogTechnologyEntry> {
        self.technologies.get(technology)
    }

    pub fn get_combo(&self, combo_id: &str) -> Option<&CatalogComboEntry> {
        self.combos.get(combo_id)
    }

    pub fn combos(&self) -> impl Iterator<Item = &CatalogComboEntry> {
        self.combos.values()
    }

    pub fn technologies(&self) -> impl Iterator<Item = (&TechnologyId, &CatalogTechnologyEntry)> {
        self.technologies.iter()
    }

    /// Iterate over all skill definitions in the catalog.
    pub fn skill_definitions(&self) -> impl Iterator<Item = &CatalogSkillDefinition> {
        self.skill_definitions.values()
    }

    /// Returns the human-readable name for a technology, falling back to the raw ID.
    pub fn technology_name<'a>(&'a self, id: &'a TechnologyId) -> &'a str {
        self.technologies
            .get(id)
            .map(|entry| entry.name.as_str())
            .unwrap_or_else(|| id.as_ref())
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddedSkillCatalog(ResolvedSkillCatalog);

impl Default for EmbeddedSkillCatalog {
    fn default() -> Self {
        Self(
            parse_embedded_catalog(EMBEDDED_CATALOG_METADATA)
                .expect("embedded recommendation catalog must remain valid"),
        )
    }
}

impl std::ops::Deref for EmbeddedSkillCatalog {
    type Target = ResolvedSkillCatalog;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawCatalogDocument {
    version: String,
    #[serde(default)]
    skills: Vec<RawCatalogSkill>,
    #[serde(default)]
    technologies: Vec<RawCatalogTechnology>,
    #[serde(default)]
    combos: Vec<RawCatalogCombo>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawCatalogSkill {
    provider_skill_id: String,
    local_skill_id: String,
    title: String,
    summary: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawCatalogTechnology {
    id: String,
    name: String,
    skills: Vec<String>,
    #[serde(default)]
    detect: Option<DetectionRules>,
    #[serde(default)]
    min_confidence: Option<String>,
    #[serde(default)]
    reason_template: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawCatalogCombo {
    id: String,
    name: String,
    requires: Vec<String>,
    skills: Vec<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    reason_template: Option<String>,
}

impl From<ProviderCatalogMetadata> for RawCatalogDocument {
    fn from(metadata: ProviderCatalogMetadata) -> Self {
        Self {
            version: metadata.schema_version,
            skills: metadata
                .skills
                .into_iter()
                .map(|skill| RawCatalogSkill {
                    provider_skill_id: skill.provider_skill_id,
                    local_skill_id: skill.local_skill_id,
                    title: skill.title,
                    summary: skill.summary,
                })
                .collect(),
            technologies: metadata
                .technologies
                .into_iter()
                .map(|technology| RawCatalogTechnology {
                    id: technology.id,
                    name: technology.name,
                    skills: technology.skills,
                    detect: technology.detect,
                    min_confidence: technology.min_confidence,
                    reason_template: technology.reason_template,
                })
                .collect(),
            combos: metadata
                .combos
                .into_iter()
                .map(|combo| RawCatalogCombo {
                    id: combo.id,
                    name: combo.name,
                    requires: combo.requires,
                    skills: combo.skills,
                    enabled: combo.enabled,
                    reason_template: combo.reason_template,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValidationMode {
    Strict,
    Lenient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EmbeddedRecommendationSource {
    LocalCurated,
    ApprovedExternal,
    DisallowedExternal,
}

pub fn parse_embedded_catalog(metadata: &str) -> Result<ResolvedSkillCatalog> {
    let document = toml::from_str::<RawCatalogDocument>(metadata)
        .context("failed to parse embedded recommendation catalog metadata")?;

    let catalog = normalize_catalog(
        EMBEDDED_SOURCE_NAME,
        EMBEDDED_METADATA_VERSION,
        document,
        ValidationMode::Strict,
    )?;

    validate_embedded_external_recommendation_policy(&catalog)
        .context("embedded recommendation catalog policy validation failed")?;

    Ok(catalog)
}

pub fn parse_catalog(
    metadata: &str,
    source_name: &str,
    metadata_version: &str,
) -> Result<ResolvedSkillCatalog> {
    let document = toml::from_str::<RawCatalogDocument>(metadata)?;
    normalize_catalog(
        source_name,
        metadata_version,
        document,
        ValidationMode::Strict,
    )
}

pub fn load_catalog(provider: Option<&dyn Provider>) -> Result<ResolvedSkillCatalog> {
    let baseline = parse_embedded_catalog(EMBEDDED_CATALOG_METADATA)
        .context("failed to initialize embedded recommendation catalog")?;

    let Some(provider) = provider else {
        return Ok(baseline);
    };

    let provider_metadata = match provider.recommendation_catalog() {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return Ok(baseline),
        Err(error) => {
            warn!(error = %error, "Falling back to embedded recommendation catalog");
            return Ok(baseline);
        }
    };

    match overlay_catalog(baseline.clone(), provider_metadata) {
        Ok(Some(catalog)) => Ok(catalog),
        Ok(None) => Ok(baseline),
        Err(error) => {
            warn!(error = %error, "Ignoring invalid provider recommendation catalog overlay");
            Ok(baseline)
        }
    }
}

pub fn recommend_skills(
    catalog: &ResolvedSkillCatalog,
    detections: &[TechnologyDetection],
) -> Vec<SkillSuggestion> {
    let mut suggestions = BTreeMap::<String, SkillSuggestion>::new();

    // Phase 1: Individual technology-based recommendations.
    for detection in detections {
        let Some(entry) = catalog.get_technology(&detection.technology) else {
            continue;
        };

        if detection.confidence < entry.min_confidence {
            continue;
        }

        for provider_skill_id in &entry.skills {
            let Some(definition) = catalog.get_skill_definition(provider_skill_id) else {
                continue;
            };

            let metadata = CatalogSkillMetadata {
                provider_skill_id: definition.provider_skill_id.clone(),
                skill_id: definition.local_skill_id.clone(),
                title: definition.title.clone(),
                summary: definition.summary.clone(),
            };

            let suggestion = suggestions
                .entry(definition.local_skill_id.clone())
                .or_insert_with(|| SkillSuggestion::new(&metadata, catalog));

            suggestion.add_match(detection, &entry.reason_template);
        }
    }

    // Phase 2: Combo-based recommendations.
    // Evaluate all enabled combos — if every required technology was detected,
    // add the combo's skills to the suggestion set.
    for combo in catalog.combos() {
        if !combo.enabled {
            continue;
        }

        let all_required_detected = combo
            .requires
            .iter()
            .all(|required_tech| detections.iter().any(|d| d.technology == *required_tech));

        if !all_required_detected {
            continue;
        }

        for provider_skill_id in &combo.skills {
            let Some(definition) = catalog.get_skill_definition(provider_skill_id) else {
                continue;
            };

            let metadata = CatalogSkillMetadata {
                provider_skill_id: definition.provider_skill_id.clone(),
                skill_id: definition.local_skill_id.clone(),
                title: definition.title.clone(),
                summary: definition.summary.clone(),
            };

            let suggestion = suggestions
                .entry(definition.local_skill_id.clone())
                .or_insert_with(|| SkillSuggestion::new(&metadata, catalog));

            let reason = combo
                .reason_template
                .as_deref()
                .unwrap_or("Recommended for {combo_name} combination.")
                .replace("{combo_name}", &combo.name);
            suggestion.add_combo_match(&combo.name, &reason);
        }
    }

    suggestions.into_values().collect()
}

pub fn overlay_catalog(
    mut baseline: ResolvedSkillCatalog,
    metadata: ProviderCatalogMetadata,
) -> Result<Option<ResolvedSkillCatalog>> {
    let provider_name = metadata.provider.clone();
    let provider_version = metadata.version.clone();
    let document = RawCatalogDocument::from(metadata);

    validate_schema_version(&document.version)?;

    let provider_skills = normalize_skill_definitions(
        &provider_name,
        &document.skills,
        ValidationMode::Lenient,
        Some(&baseline.skill_definitions),
    )?;

    let skill_view = merged_skill_view(&baseline.skill_definitions, &provider_skills);
    let provider_technologies = normalize_technologies(
        &provider_name,
        &document.technologies,
        ValidationMode::Lenient,
        &skill_view,
    )?;
    let provider_combos = normalize_combos(
        &provider_name,
        &document.combos,
        ValidationMode::Lenient,
        &skill_view,
    )?;

    let changed = !provider_skills.is_empty()
        || !provider_technologies.is_empty()
        || !provider_combos.is_empty();
    if !changed {
        return Ok(None);
    }

    for (provider_skill_id, definition) in provider_skills {
        baseline
            .skill_definitions
            .insert(provider_skill_id, definition);
    }
    rebuild_local_skill_index(&mut baseline)?;

    for (technology_id, technology) in provider_technologies {
        baseline.technologies.insert(technology_id, technology);
    }

    for (combo_id, combo) in provider_combos {
        baseline.combos.insert(combo_id, combo);
    }

    baseline.source_name = provider_name;
    baseline.metadata_version = provider_version;
    Ok(Some(baseline))
}

fn normalize_catalog(
    source_name: &str,
    metadata_version: &str,
    document: RawCatalogDocument,
    mode: ValidationMode,
) -> Result<ResolvedSkillCatalog> {
    validate_schema_version(&document.version)?;

    let skill_definitions = normalize_skill_definitions(source_name, &document.skills, mode, None)?;
    let technologies = normalize_technologies(
        source_name,
        &document.technologies,
        mode,
        &skill_definitions,
    )?;
    let combos = normalize_combos(source_name, &document.combos, mode, &skill_definitions)?;

    let mut catalog = ResolvedSkillCatalog {
        source_name: source_name.to_string(),
        metadata_version: metadata_version.to_string(),
        skill_definitions,
        local_skills: BTreeMap::new(),
        technologies,
        combos,
    };
    rebuild_local_skill_index(&mut catalog)?;
    Ok(catalog)
}

fn validate_embedded_external_recommendation_policy(catalog: &ResolvedSkillCatalog) -> Result<()> {
    for (technology_id, technology) in &catalog.technologies {
        for provider_skill_id in &technology.skills {
            validate_embedded_recommendation_reference(
                provider_skill_id,
                &format!("technology '{}'", technology_id.as_ref()),
            )?;
        }
    }

    for combo in catalog.combos.values() {
        for provider_skill_id in &combo.skills {
            validate_embedded_recommendation_reference(
                provider_skill_id,
                &format!("combo '{}'", combo.id),
            )?;
        }
    }

    Ok(())
}

fn validate_embedded_recommendation_reference(provider_skill_id: &str, owner: &str) -> Result<()> {
    if classify_embedded_recommendation_source(provider_skill_id)
        == EmbeddedRecommendationSource::DisallowedExternal
    {
        bail!("{owner} references disallowed external recommendation '{provider_skill_id}'");
    }

    Ok(())
}

fn classify_embedded_recommendation_source(
    provider_skill_id: &str,
) -> EmbeddedRecommendationSource {
    if provider_skill_id.starts_with(LOCAL_EMBEDDED_SKILL_PREFIX) {
        return EmbeddedRecommendationSource::LocalCurated;
    }

    if APPROVED_EMBEDDED_EXTERNAL_SKILL_IDS.contains(&provider_skill_id) {
        return EmbeddedRecommendationSource::ApprovedExternal;
    }

    EmbeddedRecommendationSource::DisallowedExternal
}

fn validate_schema_version(version: &str) -> Result<()> {
    if version == SUPPORTED_SCHEMA_VERSION {
        Ok(())
    } else {
        bail!(
            "unsupported recommendation catalog schema version: expected {SUPPORTED_SCHEMA_VERSION}, got {version}"
        )
    }
}

fn normalize_skill_definitions(
    source_name: &str,
    skills: &[RawCatalogSkill],
    mode: ValidationMode,
    existing: Option<&BTreeMap<String, CatalogSkillDefinition>>,
) -> Result<BTreeMap<String, CatalogSkillDefinition>> {
    let mut normalized = BTreeMap::new();
    let mut aliases = existing
        .map(|definitions| {
            definitions
                .iter()
                .map(|(provider_skill_id, definition)| {
                    (definition.local_skill_id.clone(), provider_skill_id.clone())
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    for raw_skill in skills {
        let result = normalize_skill_definition(raw_skill).and_then(|definition| {
            if let Some(existing_provider_skill_id) = aliases.get(&definition.local_skill_id)
                && existing_provider_skill_id != &definition.provider_skill_id
            {
                bail!(
                    "duplicate local skill alias '{}' is already owned by '{}'",
                    definition.local_skill_id,
                    existing_provider_skill_id
                );
            }

            if let Some((existing_local_alias, _)) =
                aliases.iter().find(|(local_alias, provider_skill_id)| {
                    *provider_skill_id == &definition.provider_skill_id
                        && *local_alias != &definition.local_skill_id
                })
            {
                bail!(
                    "provider skill '{}' is already owned by local alias '{}'",
                    definition.provider_skill_id,
                    existing_local_alias
                );
            }

            if normalized.contains_key(&definition.provider_skill_id) {
                bail!(
                    "duplicate provider skill definition '{}' in the same catalog document",
                    definition.provider_skill_id
                );
            }

            aliases.insert(
                definition.local_skill_id.clone(),
                definition.provider_skill_id.clone(),
            );
            Ok(definition)
        });

        match result {
            Ok(definition) => {
                normalized.insert(definition.provider_skill_id.clone(), definition);
            }
            Err(error) if mode == ValidationMode::Lenient => {
                warn!(
                    source = source_name,
                    item = "skill definition",
                    error = %error,
                    "Skipping invalid provider catalog entry"
                );
            }
            Err(error) => {
                return Err(anyhow!("invalid {source_name} skill definition: {error}"));
            }
        }
    }

    Ok(normalized)
}

fn normalize_technologies(
    source_name: &str,
    technologies: &[RawCatalogTechnology],
    mode: ValidationMode,
    known_skills: &BTreeMap<String, CatalogSkillDefinition>,
) -> Result<BTreeMap<TechnologyId, CatalogTechnologyEntry>> {
    let mut normalized = BTreeMap::new();

    for raw_technology in technologies {
        let result =
            normalize_technology_entry(raw_technology, known_skills).and_then(|technology| {
                if normalized.contains_key(&technology.id) {
                    bail!(
                        "duplicate technology entry '{}' in the same catalog document",
                        raw_technology.id
                    );
                }
                Ok(technology)
            });

        match result {
            Ok(technology) => {
                let key = technology.id.clone();
                normalized.insert(key, technology);
            }
            Err(error) if mode == ValidationMode::Lenient => {
                warn!(
                    source = source_name,
                    item = "technology entry",
                    error = %error,
                    "Skipping invalid provider catalog entry"
                );
            }
            Err(error) => {
                return Err(anyhow!("invalid {source_name} technology entry: {error}"));
            }
        }
    }

    Ok(normalized)
}

fn normalize_combos(
    source_name: &str,
    combos: &[RawCatalogCombo],
    mode: ValidationMode,
    known_skills: &BTreeMap<String, CatalogSkillDefinition>,
) -> Result<BTreeMap<String, CatalogComboEntry>> {
    let mut normalized = BTreeMap::new();

    for raw_combo in combos {
        let result = normalize_combo_entry(raw_combo, known_skills).and_then(|combo| {
            if normalized.contains_key(&combo.id) {
                bail!(
                    "duplicate combo entry '{}' in the same catalog document",
                    combo.id
                );
            }
            Ok(combo)
        });

        match result {
            Ok(combo) => {
                normalized.insert(combo.id.clone(), combo);
            }
            Err(error) if mode == ValidationMode::Lenient => {
                warn!(
                    source = source_name,
                    item = "combo entry",
                    error = %error,
                    "Skipping invalid provider catalog entry"
                );
            }
            Err(error) => {
                return Err(anyhow!("invalid {source_name} combo entry: {error}"));
            }
        }
    }

    Ok(normalized)
}

fn normalize_skill_definition(raw_skill: &RawCatalogSkill) -> Result<CatalogSkillDefinition> {
    let provider_skill_id = require_non_empty("provider_skill_id", &raw_skill.provider_skill_id)?;
    let local_skill_id = require_non_empty("local_skill_id", &raw_skill.local_skill_id)?;
    let title = require_non_empty("title", &raw_skill.title)?;
    let summary = require_non_empty("summary", &raw_skill.summary)?;
    validate_local_skill_id(local_skill_id)?;

    Ok(CatalogSkillDefinition {
        provider_skill_id: provider_skill_id.to_string(),
        local_skill_id: local_skill_id.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
    })
}

fn normalize_technology_entry(
    raw_technology: &RawCatalogTechnology,
    known_skills: &BTreeMap<String, CatalogSkillDefinition>,
) -> Result<CatalogTechnologyEntry> {
    let technology_id = require_non_empty("technology.id", &raw_technology.id)?;
    let id = TechnologyId::new(technology_id);
    let name = require_non_empty("technology.name", &raw_technology.name)?;
    let skills =
        normalize_skill_references("technology.skills", &raw_technology.skills, known_skills)?;
    let min_confidence = raw_technology
        .min_confidence
        .as_deref()
        .map(|value| {
            DetectionConfidence::from_catalog_key(value)
                .ok_or_else(|| anyhow!("invalid minimum confidence '{value}'"))
        })
        .transpose()?
        .unwrap_or(DetectionConfidence::Medium);
    let reason_template = raw_technology
        .reason_template
        .as_deref()
        .unwrap_or(DEFAULT_REASON_TEMPLATE)
        .to_string();

    Ok(CatalogTechnologyEntry {
        id,
        name: name.to_string(),
        detect: raw_technology.detect.clone(),
        skills,
        min_confidence,
        reason_template,
    })
}

fn normalize_combo_entry(
    raw_combo: &RawCatalogCombo,
    known_skills: &BTreeMap<String, CatalogSkillDefinition>,
) -> Result<CatalogComboEntry> {
    let combo_id = require_non_empty("combo.id", &raw_combo.id)?;
    let name = require_non_empty("combo.name", &raw_combo.name)?;
    if raw_combo.requires.is_empty() {
        bail!("combo.requires must include at least one technology id");
    }

    let mut requires = Vec::new();
    for required in &raw_combo.requires {
        let required = require_non_empty("combo.requires", required)?;
        let technology = TechnologyId::new(required);
        if !requires.contains(&technology) {
            requires.push(technology);
        }
    }

    let skills = normalize_skill_references("combo.skills", &raw_combo.skills, known_skills)?;

    Ok(CatalogComboEntry {
        id: combo_id.to_string(),
        name: name.to_string(),
        requires,
        skills,
        enabled: raw_combo.enabled.unwrap_or(false),
        reason_template: raw_combo.reason_template.clone(),
    })
}

fn normalize_skill_references(
    field_name: &str,
    skills: &[String],
    known_skills: &BTreeMap<String, CatalogSkillDefinition>,
) -> Result<Vec<String>> {
    if skills.is_empty() {
        bail!("{field_name} must include at least one skill reference");
    }

    let mut references = Vec::new();
    for provider_skill_id in skills {
        let provider_skill_id = require_non_empty(field_name, provider_skill_id)?;
        if !known_skills.contains_key(provider_skill_id) {
            bail!("{field_name} references unknown skill '{provider_skill_id}'");
        }
        if !references
            .iter()
            .any(|existing| existing == provider_skill_id)
        {
            references.push(provider_skill_id.to_string());
        }
    }

    Ok(references)
}

fn rebuild_local_skill_index(catalog: &mut ResolvedSkillCatalog) -> Result<()> {
    let mut local_skills = BTreeMap::new();

    for definition in catalog.skill_definitions.values() {
        if local_skills.contains_key(&definition.local_skill_id) {
            bail!(
                "duplicate local skill alias '{}' after applying catalog overlay",
                definition.local_skill_id
            );
        }

        local_skills.insert(
            definition.local_skill_id.clone(),
            CatalogSkillMetadata {
                provider_skill_id: definition.provider_skill_id.clone(),
                skill_id: definition.local_skill_id.clone(),
                title: definition.title.clone(),
                summary: definition.summary.clone(),
            },
        );
    }

    catalog.local_skills = local_skills;
    Ok(())
}

fn merged_skill_view<'a>(
    baseline: &'a BTreeMap<String, CatalogSkillDefinition>,
    overlay: &'a BTreeMap<String, CatalogSkillDefinition>,
) -> BTreeMap<String, CatalogSkillDefinition> {
    let mut merged = baseline.clone();
    merged.extend(overlay.clone());
    merged
}

fn require_non_empty<'a>(field_name: &str, value: &'a str) -> Result<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{field_name} must not be empty");
    }
    Ok(trimmed)
}

fn validate_local_skill_id(skill_id: &str) -> Result<()> {
    if skill_id.contains('/') || skill_id.contains('\\') {
        bail!("local skill id must be a single path-safe segment");
    }

    let path = Path::new(skill_id);
    if path.is_absolute() {
        bail!("local skill id must not be an absolute path");
    }

    let mut component_count = 0usize;
    for component in path.components() {
        match component {
            Component::Normal(_) => component_count += 1,
            _ => bail!("local skill id must be a single path-safe segment"),
        }
    }

    if component_count != 1 {
        bail!("local skill id must be a single path-safe segment");
    }

    Ok(())
}
