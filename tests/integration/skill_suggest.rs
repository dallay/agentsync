use agentsync::skills::provider::{
    Provider, ProviderCatalogMetadata, ProviderCatalogSkill, ProviderCatalogTechnology,
    SkillInstallInfo,
};
use agentsync::skills::suggest::{
    DetectionConfidence, DetectionEvidence, SuggestionService, TechnologyDetection, TechnologyId,
};
use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn agentsync_bin() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

#[test]
fn skill_suggest_json_is_read_only_and_marks_installed_skills() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    let registry_path = skills_dir.join("registry.json");
    let registry_body = serde_json::json!({
        "schemaVersion": 1,
        "last_updated": "2026-03-30T00:00:00Z",
        "skills": {
            "docker-expert": {
                "name": "docker-expert",
                "version": "1.2.3"
            }
        }
    });
    let registry_body = serde_json::to_string_pretty(&registry_body).unwrap();
    fs::write(&registry_path, &registry_body).unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let recommendations = response["recommendations"].as_array().unwrap();
    let docker = recommendations
        .iter()
        .find(|recommendation| recommendation["skill_id"] == "docker-expert")
        .unwrap();
    assert_eq!(docker["installed"], true);

    let registry_after = fs::read_to_string(&registry_path).unwrap();
    assert_eq!(registry_after, registry_body);
    assert!(!skills_dir.join("rust-async-patterns").exists());
}

#[test]
fn skill_suggest_recommends_python_backend_frameworks_from_python_dependency_files() {
    struct Case {
        name: &'static str,
        files: &'static [(&'static str, &'static str)],
        expected_technology: &'static str,
        expected_skill: &'static str,
    }

    let cases = [
        Case {
            name: "python-baseline-pyproject",
            files: &[("pyproject.toml", "[project]\ndependencies = []\n")],
            expected_technology: "python",
            expected_skill: "python-executor",
        },
        Case {
            name: "fastapi-requirements",
            files: &[("requirements.txt", "fastapi[standard]==0.115.0\n")],
            expected_technology: "fastapi",
            expected_skill: "fastapi-templates",
        },
        Case {
            name: "django-pyproject",
            files: &[(
                "pyproject.toml",
                "[project]\ndependencies = [\"Django>=4.2\"]\n",
            )],
            expected_technology: "django",
            expected_skill: "django-expert",
        },
        Case {
            name: "flask-pipfile",
            files: &[("Pipfile", "[packages]\nFlask = \"*\"\n")],
            expected_technology: "flask",
            expected_skill: "flask-api-development",
        },
        Case {
            name: "pydantic-pyproject-poetry",
            files: &[(
                "pyproject.toml",
                "[tool.poetry.dependencies]\npython = \"^3.12\"\npydantic = \"^2\"\n",
            )],
            expected_technology: "pydantic",
            expected_skill: "pydantic",
        },
        Case {
            name: "sqlalchemy-requirements",
            files: &[("requirements.txt", "SQLAlchemy>=2\n")],
            expected_technology: "sqlalchemy",
            expected_skill: "sqlalchemy",
        },
        Case {
            name: "pytest-pyproject-optional-deps",
            files: &[(
                "pyproject.toml",
                "[project.optional-dependencies]\ntest = [\"pytest>=8\"]\n",
            )],
            expected_technology: "pytest",
            expected_skill: "python-testing-patterns",
        },
        Case {
            name: "pandas-requirements",
            files: &[("requirements.txt", "pandas==2.2.0\n")],
            expected_technology: "pandas",
            expected_skill: "pandas-pro",
        },
        Case {
            name: "numpy-requirements",
            files: &[("requirements.txt", "numpy==2.0.0\n")],
            expected_technology: "numpy",
            expected_skill: "machine-learning",
        },
        Case {
            name: "scikit-learn-requirements",
            files: &[("requirements.txt", "scikit-learn==1.5.0\n")],
            expected_technology: "scikit_learn",
            expected_skill: "scikit-learn",
        },
        Case {
            name: "celery-pipfile",
            files: &[(
                "Pipfile",
                "[packages]\ncelery = { version = \"*\", extras = [\"redis\"] }\n",
            )],
            expected_technology: "celery",
            expected_skill: "python-background-jobs",
        },
        Case {
            name: "requests-editable-requirements",
            files: &[(
                "requirements.txt",
                "-e git+https://example.com/client.git#egg=requests\n",
            )],
            expected_technology: "requests",
            expected_skill: "python-patterns",
        },
    ];

    for case in cases {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        for (path, contents) in case.files {
            let path = root.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }

        let output = Command::new(agentsync_bin())
            .current_dir(root)
            .args(["skill", "suggest", "--json"])
            .output()
            .unwrap_or_else(|error| panic!("failed to run agentsync for {}: {error}", case.name));

        assert!(
            output.status.success(),
            "{} stderr: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );

        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let detections = response["detections"].as_array().unwrap();
        assert!(
            detections
                .iter()
                .any(|detection| detection["technology"] == case.expected_technology),
            "{} should detect {}. response: {}",
            case.name,
            case.expected_technology,
            response
        );

        let recommendations = response["recommendations"].as_array().unwrap();
        assert!(
            recommendations
                .iter()
                .any(|recommendation| recommendation["skill_id"] == case.expected_skill),
            "{} should recommend {}. response: {}",
            case.name,
            case.expected_skill,
            response
        );
    }
}

#[test]
fn skill_suggest_recommends_js_frontend_autoskills_parity() {
    struct Case {
        name: &'static str,
        files: &'static [(&'static str, &'static str)],
        expected_technology: &'static str,
        expected_skill: &'static str,
    }

    let cases = [
        Case {
            name: "react-hook-form-package-json",
            files: &[(
                "package.json",
                r#"{"dependencies":{"react-hook-form":"^7.0.0"}}"#,
            )],
            expected_technology: "react-hook-form",
            expected_skill: "react-hook-form",
        },
        Case {
            name: "zod-package-json",
            files: &[("package.json", r#"{"dependencies":{"zod":"^3.0.0"}}"#)],
            expected_technology: "zod",
            expected_skill: "zod",
        },
        Case {
            name: "tanstack-start-package-json",
            files: &[(
                "package.json",
                r#"{"dependencies":{"@tanstack/react-start":"^1.0.0"}}"#,
            )],
            expected_technology: "tanstack-start",
            expected_skill: "tanstack-start",
        },
        Case {
            name: "chrome-extension-manifest",
            files: &[("manifest.json", r#"{"manifest_version":3,"name":"Demo"}"#)],
            expected_technology: "chrome-extension",
            expected_skill: "chrome-extension-development",
        },
        Case {
            name: "bun-lockfile",
            files: &[("bun.lock", "")],
            expected_technology: "bun",
            expected_skill: "bun",
        },
        Case {
            name: "threejs-package-json",
            files: &[("package.json", r#"{"dependencies":{"three":"^0.170.0"}}"#)],
            expected_technology: "threejs",
            expected_skill: "threejs-fundamentals",
        },
        Case {
            name: "react-three-fiber-package-json",
            files: &[(
                "package.json",
                r#"{"dependencies":{"@react-three/fiber":"^9.0.0"}}"#,
            )],
            expected_technology: "react-three-fiber",
            expected_skill: "react-three-fiber",
        },
    ];

    for case in cases {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        for (path, contents) in case.files {
            let path = root.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }

        let output = Command::new(agentsync_bin())
            .current_dir(root)
            .args(["skill", "suggest", "--json"])
            .output()
            .unwrap_or_else(|error| panic!("failed to run agentsync for {}: {error}", case.name));

        assert!(
            output.status.success(),
            "{} stderr: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );

        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let detections = response["detections"].as_array().unwrap();
        assert!(
            detections
                .iter()
                .any(|detection| detection["technology"] == case.expected_technology),
            "{} should detect {}. response: {}",
            case.name,
            case.expected_technology,
            response
        );

        let recommendations = response["recommendations"].as_array().unwrap();
        assert!(
            recommendations
                .iter()
                .any(|recommendation| recommendation["skill_id"] == case.expected_skill),
            "{} should recommend {}. response: {}",
            case.name,
            case.expected_skill,
            response
        );
    }
}

#[test]
fn skill_suggest_recommends_autoskills_frontend_parity_combos() {
    struct Case {
        name: &'static str,
        files: &'static [(&'static str, &'static str)],
        expected_skills: &'static [&'static str],
    }

    let cases = [
        Case {
            name: "react-hook-form-zod",
            files: &[(
                "package.json",
                r#"{"dependencies":{"react-hook-form":"latest","zod":"latest"}}"#,
            )],
            expected_skills: &["react-hook-form", "zod"],
        },
        Case {
            name: "nextjs-vercel-ai",
            files: &[(
                "package.json",
                r#"{"dependencies":{"next":"latest","ai":"latest"}}"#,
            )],
            expected_skills: &["use-ai-sdk", "next-best-practices"],
        },
        Case {
            name: "react-shadcn",
            files: &[
                (
                    "package.json",
                    r#"{"dependencies":{"react":"latest","react-dom":"latest"}}"#,
                ),
                ("components.json", "{}"),
            ],
            expected_skills: &["shadcn", "vercel-react-best-practices"],
        },
        Case {
            name: "tailwind-shadcn",
            files: &[
                (
                    "package.json",
                    r#"{"dependencies":{"tailwindcss":"latest"}}"#,
                ),
                ("components.json", "{}"),
            ],
            expected_skills: &["tailwind-v4-shadcn"],
        },
        Case {
            name: "cloudflare-vite",
            files: &[
                (
                    "package.json",
                    r#"{"dependencies":{"vite":"latest","wrangler":"latest"}}"#,
                ),
                ("wrangler.toml", "name = 'demo'\n"),
            ],
            expected_skills: &["migrate-to-vinext"],
        },
        Case {
            name: "node-express",
            files: &[("package.json", r#"{"dependencies":{"express":"latest"}}"#)],
            expected_skills: &["nodejs-express-server"],
        },
        Case {
            name: "react-three-fiber",
            files: &[(
                "package.json",
                r#"{"dependencies":{"react":"latest","react-dom":"latest","three":"latest","@react-three/fiber":"latest"}}"#,
            )],
            expected_skills: &["react-three-fiber"],
        },
        Case {
            name: "react-clerk",
            files: &[(
                "package.json",
                r#"{"dependencies":{"react":"latest","react-dom":"latest","@clerk/clerk-react":"latest"}}"#,
            )],
            expected_skills: &["clerk-react-patterns"],
        },
        Case {
            name: "nuxt-clerk",
            files: &[(
                "package.json",
                r#"{"dependencies":{"nuxt":"latest","@clerk/nuxt":"latest"}}"#,
            )],
            expected_skills: &["clerk-nuxt-patterns"],
        },
        Case {
            name: "vue-clerk",
            files: &[(
                "package.json",
                r#"{"dependencies":{"vue":"latest","@clerk/vue":"latest"}}"#,
            )],
            expected_skills: &["clerk-vue-patterns"],
        },
        Case {
            name: "astro-clerk",
            files: &[(
                "package.json",
                r#"{"dependencies":{"astro":"latest","@clerk/astro":"latest"}}"#,
            )],
            expected_skills: &["clerk-astro-patterns"],
        },
        Case {
            name: "expo-clerk",
            files: &[(
                "package.json",
                r#"{"dependencies":{"expo":"latest","@clerk/clerk-expo":"latest"}}"#,
            )],
            expected_skills: &["clerk-expo-patterns"],
        },
        Case {
            name: "react-router-clerk",
            files: &[(
                "package.json",
                r#"{"dependencies":{"react-router":"latest","@clerk/react-router":"latest"}}"#,
            )],
            expected_skills: &["clerk-react-router-patterns"],
        },
        Case {
            name: "tanstack-clerk",
            files: &[(
                "package.json",
                r#"{"dependencies":{"@tanstack/react-start":"latest","@clerk/tanstack-react-start":"latest"}}"#,
            )],
            expected_skills: &["clerk-tanstack-patterns"],
        },
        Case {
            name: "chrome-extension-clerk",
            files: &[
                (
                    "package.json",
                    r#"{"dependencies":{"@clerk/chrome-extension":"latest"}}"#,
                ),
                (
                    "manifest.json",
                    r#"{"manifest_version":3,"name":"Demo","version":"1.0.0"}"#,
                ),
            ],
            expected_skills: &["clerk-chrome-extension-patterns"],
        },
    ];

    for case in cases {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        for (path, contents) in case.files {
            let path = root.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }

        let output = Command::new(agentsync_bin())
            .current_dir(root)
            .args(["skill", "suggest", "--json"])
            .output()
            .unwrap_or_else(|error| panic!("failed to run agentsync for {}: {error}", case.name));

        assert!(
            output.status.success(),
            "{} stderr: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );

        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let recommendations = response["recommendations"].as_array().unwrap();
        for expected_skill in case.expected_skills {
            let matching_recommendation = recommendations
                .iter()
                .find(|recommendation| recommendation["skill_id"] == *expected_skill);

            assert!(
                matching_recommendation.is_some(),
                "{} should recommend {}. response: {}",
                case.name,
                expected_skill,
                response
            );

            let recommendation = matching_recommendation.unwrap();
            let reasons = recommendation["reasons"].as_array().unwrap();
            let has_combo_reason = reasons
                .iter()
                .any(|reason| reason.as_str().unwrap().contains("combination"));

            assert!(
                has_combo_reason,
                "{} should have combo reason for skill {}. recommendation: {}",
                case.name, expected_skill, recommendation
            );
        }
    }
}

#[test]
fn skill_suggest_human_output_reports_empty_results() {
    let temp_dir = TempDir::new().unwrap();
    let output = Command::new(agentsync_bin())
        .current_dir(temp_dir.path())
        .args(["skill", "suggest"])
        .output()
        .expect("failed to run agentsync skill suggest");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Detected technologies: none"), "{stdout}");
    assert!(stdout.contains("Recommended skills: none"), "{stdout}");
    assert!(
        stdout.contains("Summary: 0 detected, 0 recommended, 0 installable"),
        "{stdout}"
    );
}

#[test]
fn skill_suggest_install_requires_tty_without_all_flag() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--install", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --json");

    assert!(!output.status.success());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["code"], "interactive_tty_required");
    assert!(
        response["remediation"]
            .as_str()
            .unwrap()
            .contains("--install --all")
    );
}

#[test]
fn skill_suggest_install_all_installs_pending_recommendations_and_skips_installed() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();

    let source_root = root.join("skill-sources");
    create_skill_source(&source_root, "rust-async-patterns");
    create_skill_source(&source_root, "docker-expert");

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(
        skills_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "last_updated": "2026-03-30T00:00:00Z",
            "skills": {
                "docker-expert": {
                    "name": "docker-expert",
                    "version": "1.2.3"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .env("AGENTSYNC_TEST_SKILL_SOURCE_DIR", &source_root)
        .args(["skill", "suggest", "--install", "--all", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --all --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["mode"], "install_all");
    let results = response["results"].as_array().unwrap();
    let rust = results
        .iter()
        .find(|result| result["skill_id"] == "rust-async-patterns")
        .unwrap();
    let docker = results
        .iter()
        .find(|result| result["skill_id"] == "docker-expert")
        .unwrap();
    assert_eq!(rust["status"], "installed");
    assert_eq!(docker["status"], "already_installed");
    assert!(root.join(".agents/skills/rust-async-patterns").exists());
}

#[test]
fn skill_suggest_install_all_is_a_no_op_when_everything_is_already_installed() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    let registry_path = skills_dir.join("registry.json");
    let registry_body = serde_json::json!({
        "schemaVersion": 1,
        "last_updated": "2026-03-30T00:00:00Z",
        "skills": {
            "docker-expert": {
                "name": "docker-expert",
                "version": "1.2.3"
            },
            "rust-async-patterns": {
                "name": "rust-async-patterns",
                "version": "1.0.0"
            }
        }
    });
    let registry_body = serde_json::to_string_pretty(&registry_body).unwrap();
    fs::write(&registry_path, &registry_body).unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--install", "--all", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --all --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["summary"]["installable_count"], 0);

    let results = response["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(
        results
            .iter()
            .all(|result| result["status"] == "already_installed")
    );

    let registry_after = fs::read_to_string(&registry_path).unwrap();
    assert_eq!(registry_after, registry_body);
    assert!(!skills_dir.join("docker-expert").exists());
    assert!(!skills_dir.join("rust-async-patterns").exists());
}

#[test]
fn skill_suggest_install_all_surfaces_direct_install_failure_semantics() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let source_root = root.join("skill-sources");
    let failing_source = source_root.join("dallay/agents-skills/rust-async-patterns");
    fs::create_dir_all(&failing_source).unwrap();

    let suggest_output = Command::new(agentsync_bin())
        .current_dir(root)
        .env("AGENTSYNC_TEST_SKILL_SOURCE_DIR", &source_root)
        .args(["skill", "suggest", "--install", "--all", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --all --json");

    assert!(suggest_output.status.success());

    let direct_output = Command::new(agentsync_bin())
        .current_dir(root)
        .args([
            "skill",
            "install",
            "rust-async-patterns",
            "--source",
            failing_source.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("failed to run direct agentsync skill install --json");

    assert!(!direct_output.status.success());

    let suggest_response: serde_json::Value =
        serde_json::from_slice(&suggest_output.stdout).unwrap();
    let direct_response: serde_json::Value = serde_json::from_slice(&direct_output.stdout).unwrap();

    let failed_result = suggest_response["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["skill_id"] == "rust-async-patterns")
        .unwrap();

    assert_eq!(failed_result["status"], "failed");
    assert_eq!(failed_result["error_message"], direct_response["error"]);
}

#[test]
fn skill_suggest_install_all_human_output_is_line_oriented_and_readable_without_tty() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();
    fs::write(root.join("Makefile"), "all:\n\t@true\n").unwrap();

    let source_root = root.join("skill-sources");
    create_skill_source(&source_root, "docker-expert");
    create_skill_source(&source_root, "makefile");
    fs::create_dir_all(source_root.join("rust-async-patterns")).unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(
        skills_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "last_updated": "2026-03-30T00:00:00Z",
            "skills": {
                "docker-expert": {
                    "name": "docker-expert",
                    "version": "1.2.3"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .env("AGENTSYNC_TEST_SKILL_SOURCE_DIR", &source_root)
        .args(["skill", "suggest", "--install", "--all"])
        .output()
        .expect("failed to run agentsync skill suggest --install --all");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Installing 3 recommended skills..."),
        "{stdout}"
    );
    assert!(
        stdout.contains("failed rust-async-patterns during install:"),
        "{stdout}"
    );
    assert!(
        stdout.contains("already installed docker-expert"),
        "{stdout}"
    );
    assert!(stdout.contains("installed makefile"), "{stdout}");
    assert!(
        stdout.contains("Recommendation install summary"),
        "{stdout}"
    );
    assert!(stdout.contains("Installed: 1"), "{stdout}");
    assert!(stdout.contains("Already installed: 1"), "{stdout}");
    assert!(stdout.contains("Failed: 1"), "{stdout}");
    assert!(stdout.contains("Failure details:"), "{stdout}");
    assert!(stdout.contains("rust-async-patterns:"), "{stdout}");
    assert!(!stdout.contains("\u{1b}["), "{stdout}");
    assert!(!stdout.contains('\r'), "{stdout:?}");
    assert!(!stdout.contains("⠋"), "{stdout}");
}

#[test]
fn suggestion_service_preserves_local_install_lookup_with_provider_overlay() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::create_dir_all(root.join(".agents/skills")).unwrap();
    fs::write(
        root.join(".agents/skills/registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "last_updated": "2026-03-30T00:00:00Z",
            "skills": {
                "custom-rust": {
                    "name": "custom-rust",
                    "version": "1.0.0"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let detector = StaticDetector;
    let provider = CanonicalOverlayProvider;
    let response = SuggestionService
        .suggest_with(root, &detector, Some(&provider))
        .unwrap();

    let recommendation = response
        .recommendations
        .iter()
        .find(|recommendation| recommendation.skill_id == "custom-rust")
        .unwrap();

    assert!(recommendation.installed);
    assert_eq!(recommendation.installed_version.as_deref(), Some("1.0.0"));
    assert_eq!(
        recommendation.matched_technologies,
        vec![TechnologyId::new(TechnologyId::RUST)]
    );
}

fn create_skill_source(source_root: &std::path::Path, skill_id: &str) {
    let source_dir = source_root.join(skill_id);
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("SKILL.md"),
        format!("---\nname: {skill_id}\nversion: 1.0.0\n---\n# {skill_id}\n"),
    )
    .unwrap();
}

struct StaticDetector;

impl agentsync::skills::detect::RepoDetector for StaticDetector {
    fn detect(&self, _project_root: &std::path::Path) -> Result<Vec<TechnologyDetection>> {
        Ok(vec![TechnologyDetection {
            technology: TechnologyId::new(TechnologyId::RUST),
            confidence: DetectionConfidence::High,
            root_relative_paths: vec!["Cargo.toml".into()],
            evidence: vec![DetectionEvidence {
                marker: "Cargo.toml".to_string(),
                path: "Cargo.toml".into(),
                notes: None,
            }],
        }])
    }
}

struct CanonicalOverlayProvider;

impl Provider for CanonicalOverlayProvider {
    fn manifest(&self) -> Result<String> {
        Ok("canonical-overlay".to_string())
    }

    fn resolve(&self, _id: &str) -> Result<SkillInstallInfo> {
        unreachable!()
    }

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        Ok(Some(ProviderCatalogMetadata {
            provider: "canonical-overlay".to_string(),
            version: "2026.03".to_string(),
            schema_version: "v1".to_string(),
            skills: vec![ProviderCatalogSkill {
                provider_skill_id: "acme/skills/custom-rust".to_string(),
                local_skill_id: "custom-rust".to_string(),
                title: "Custom Rust".to_string(),
                summary: "Custom Rust guidance".to_string(),
                archive_subpath: None,
                legacy_local_skill_ids: Vec::new(),
                install_source: None,
            }],
            technologies: vec![ProviderCatalogTechnology {
                id: "rust".to_string(),
                name: "Rust".to_string(),
                skills: vec!["acme/skills/custom-rust".to_string()],
                detect: None,
                min_confidence: Some("medium".to_string()),
                reason_template: None,
            }],
            combos: vec![],
        }))
    }
}
