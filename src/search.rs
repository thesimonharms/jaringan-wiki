//! Full-text and semantic search across wiki pages.
//!
//! Basic full-text search is done in-process (keyword matching).
//! Semantic search uses baochuan AI when configured.

use crate::wiki::Wiki;

/// A search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub slug: String,
    pub title: String,
    pub relevance: f64,
    pub snippet: String,
}

/// Search index for wiki content.
#[derive(Debug, Clone)]
pub struct SearchIndex;

impl SearchIndex {
    /// Full-text search across wiki pages.
    pub fn search(wiki: &Wiki, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let terms: Vec<&str> = query_lower.split_whitespace().collect();
        if terms.is_empty() {
            return vec![];
        }

        let mut results: Vec<SearchResult> = wiki.pages.values()
            .filter_map(|page| {
                let body_lower = page.body.to_lowercase();
                let title_lower = page.title.to_lowercase();

                // Count matching terms
                let matches: usize = terms.iter()
                    .filter(|t| body_lower.contains(*t) || title_lower.contains(*t))
                    .count();

                if matches == 0 {
                    return None;
                }

                let relevance = if title_lower.contains(&query_lower) {
                    1.0 + (matches as f64 * 0.5)
                } else if terms.iter().all(|t| body_lower.contains(t)) {
                    0.5 + (matches as f64 * 0.3)
                } else {
                    matches as f64 * 0.2
                };

                // Generate snippet
                let snippet = generate_snippet(&page.body, &query_lower);

                Some(SearchResult {
                    slug: page.slug.clone(),
                    title: page.title.clone(),
                    relevance,
                    snippet,
                })
            })
            .collect();

        results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(20);
        results
    }

    /// Generate a text snippet around the first match.
    pub fn snippet_for(wiki: &Wiki, slug: &str) -> Option<String> {
        let page = wiki.pages.get(slug)?;
        Some(generate_snippet(&page.body, ""))
    }
}

fn generate_snippet(body: &str, query: &str) -> String {
    if query.is_empty() {
        // First ~200 chars
        let clean: String = body.chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect();
        let lines: Vec<&str> = clean.lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .collect();
        return lines.iter()
            .take(3)
            .map(|l| l.trim().to_owned())
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(200)
            .collect();
    }

    // Find first line containing the query
    for line in body.lines() {
        if line.to_lowercase().contains(query) {
            let trimmed = line.trim();
            let chars: Vec<char> = trimmed.chars().collect();
            if chars.len() <= 160 {
                return trimmed.to_owned();
            }
            // Find query position and take surrounding context
            if let Some(pos) = trimmed.to_lowercase().find(query) {
                let start = pos.saturating_sub(40);
                let end = (pos + query.len() + 40).min(chars.len());
                return format!("...{}...", &trimmed[start..end]);
            }
            return format!("{}...", &trimmed[..160]);
        }
    }

    String::new()
}
