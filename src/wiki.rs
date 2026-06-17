//! Wiki content model — pages stored as JRG files, served over the JRG protocol.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use anyhow::{Context, Result};

/// Configuration loaded from `wiki.yaml` at the wiki root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiConfig {
    /// Wiki title (shown in page headers).
    #[serde(default = "default_title")]
    pub title: String,
    /// Author / maintainer name.
    #[serde(default)]
    pub author: String,
    /// Whether to sign pages with a Jaringan key.
    #[serde(default)]
    pub signing_key: Option<String>,
    /// Path to the AI provider config (for baochuan integration).
    #[serde(default)]
    pub ai_config_path: Option<String>,
}

fn default_title() -> String { "My Wiki".to_owned() }

impl Default for WikiConfig {
    fn default() -> Self {
        Self {
            title: default_title(),
            author: String::new(),
            signing_key: None,
            ai_config_path: None,
        }
    }
}

/// A single wiki page parsed from markdown.
#[derive(Debug, Clone)]
pub struct WikiPage {
    /// URL-safe slug (derived from filename without extension).
    pub slug: String,
    /// Human-readable title (from first `# heading` or filename).
    pub title: String,
    /// Raw markdown content.
    pub body: String,
    /// List of tags extracted from front-matter.
    pub tags: Vec<String>,
    /// Links to other wiki pages `[[slug]]` or `[text](slug)`.
    pub links: Vec<String>,
    /// ISO 8601 last-modified timestamp.
    pub updated_at: String,
    /// File path on disk.
    pub path: PathBuf,
}

/// A loaded wiki — a directory of markdown pages.
#[derive(Debug, Clone)]
pub struct Wiki {
    /// Root directory containing wiki pages.
    pub root: PathBuf,
    /// Wiki configuration.
    pub config: WikiConfig,
    /// All pages indexed by slug.
    pub pages: HashMap<String, WikiPage>,
}

impl Wiki {
    /// Load a wiki from the given directory.
    pub fn load(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let root = fs::canonicalize(&root)
            .with_context(|| format!("wiki root not found: {}", root.display()))?;

        // Load config
        let config_path = root.join("wiki.yaml");
        let config = if config_path.exists() {
            let yaml = fs::read_to_string(&config_path)
                .context("failed to read wiki.yaml")?;
            serde_yaml::from_str(&yaml).context("failed to parse wiki.yaml")?
        } else {
            WikiConfig::default()
        };

        // Scan for .jrg files (skip README.jrg and wiki.yaml)
        let mut pages = HashMap::new();
        let entries = fs::read_dir(&root)
            .context("failed to read wiki directory")?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only read .jrg files (skip README.jrg, skip wiki.yaml)
            if path.extension().and_then(|s| s.to_str()) != Some("jrg") {
                continue;
            }
            let filename = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if filename == "README" || filename == "wiki" {
                continue;
            }

            let body = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;

            let (title, tags, body_clean) = parse_page(&body, filename);
            let links = extract_links(&body);
            let updated_at = path_modified(&path);

            let page = WikiPage {
                slug: filename.to_owned(),
                title,
                tags,
                links,
                body: body_clean,
                updated_at,
                path,
            };

            pages.insert(page.slug.clone(), page);
        }

        Ok(Self { root, config, pages })
    }

    /// Reload a single page by slug.
    pub fn reload_page(&mut self, slug: &str) -> Result<()> {
        let path = self.root.join(format!("{slug}.jrg"));
        if !path.exists() {
            self.pages.remove(slug);
            return Ok(());
        }
        let body = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let (title, tags, body_clean) = parse_page(&body, slug);
        let links = extract_links(&body);
        let updated_at = path_modified(&path);

        self.pages.insert(slug.to_owned(), WikiPage {
            slug: slug.to_owned(),
            title,
            tags,
            links,
            body: body_clean,
            updated_at,
            path,
        });
        Ok(())
    }

    /// Create a new wiki page.
    pub fn create_page(&self, slug: &str, title: &str, body: &str) -> Result<()> {
        let path = self.root.join(format!("{slug}.jrg"));
        let content = format!(
            "---\ntitle: {title}\ntags: []\n---\n\n# {title}\n\n{body}\n"
        );
        fs::write(&path, content)
            .with_context(|| format!("failed to create {}", path.display()))?;
        Ok(())
    }

    /// Get all page slugs and titles for navigation.
    pub fn index(&self) -> Vec<(&str, &str)> {
        let mut entries: Vec<_> = self.pages.values()
            .map(|p| (p.slug.as_str(), p.title.as_str()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        entries
    }
}

/// Parse front-matter and extract title from a markdown page.
fn parse_page(body: &str, fallback_slug: &str) -> (String, Vec<String>, String) {
    // Try YAML front-matter between --- markers
    let trimmed = body.trim();
    if trimmed.starts_with("---") {
        if let Some(end) = trimmed[3..].find("---") {
            let fm_str = &trimmed[3..3 + end];
            let rest = trimmed[3 + end + 3..].trim();

            // Parse title from front-matter
            let title = fm_str.lines()
                .find_map(|l| l.strip_prefix("title:"))
                .map(|s| s.trim().trim_matches('"').to_owned())
                .or_else(|| extract_title_from_heading(rest));

            // Parse tags from front-matter
            let tags = fm_str.lines()
                .find_map(|l| l.strip_prefix("tags:"))
                .map(|s| {
                    s.trim()
                        .trim_matches(|c: char| c == '[' || c == ']' || c == '"')
                        .split(',')
                        .map(|t| t.trim().trim_matches('"').to_owned())
                        .filter(|t| !t.is_empty())
                        .collect()
                })
                .unwrap_or_default();

            return (title.unwrap_or_else(|| fallback_slug.to_owned()), tags, rest.to_owned());
        }
    }

    // No front-matter: try first heading
    let title = extract_title_from_heading(trimmed)
        .unwrap_or_else(|| fallback_slug.to_owned());
    (title, vec![], trimmed.to_owned())
}

fn extract_title_from_heading(text: &str) -> Option<String> {
    text.lines()
        .find(|l| l.starts_with("# ") || l.starts_with("#\t"))
        .map(|l| l.trim_start_matches("# ").trim_start_matches("#\t").to_owned())
}

/// Extract wiki links `[[slug]]` from body text.
fn extract_links(body: &str) -> Vec<String> {
    let mut links = Vec::new();
    // Match [[slug]] or [[slug|text]]
    for part in body.split("[[") {
        if let Some(end) = part.find("]]") {
            let inner = &part[..end];
            let slug = inner.split('|').next().unwrap_or(inner).trim();
            links.push(slug.to_owned());
        }
    }
    links
}

/// Get the last-modified timestamp of a file.
fn path_modified(path: &Path) -> String {
    let ts = fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .unwrap_or(UNIX_EPOCH);
    let dt: DateTime<Utc> = ts.into();
    dt.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_page_with_frontmatter() {
        let md = "---\ntitle: Hello World\ntags: [rust, wiki]\n---\n\n# Hello\n\nBody text.";
        let (title, tags, body) = parse_page(md, "test");
        assert_eq!(title, "Hello World");
        assert_eq!(tags, vec!["rust", "wiki"]);
        assert!(body.contains("Body text."));
    }

    #[test]
    fn test_parse_page_no_frontmatter() {
        let md = "# Just a heading\n\nSome content.";
        let (title, tags, _body) = parse_page(md, "my-page");
        assert_eq!(title, "Just a heading");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_extract_links() {
        let md = "See [[getting-started]] and [[api/overview|API Overview]].";
        let links = extract_links(md);
        assert_eq!(links, vec!["getting-started", "api/overview"]);
    }
}
