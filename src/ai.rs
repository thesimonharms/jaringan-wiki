//! AI integration for wiki features using baochuan.
//!
//! Provides semantic search, page summarisation, Q&A, and auto-linking.

use anyhow::Result;

/// AI-powered wiki features, backed by a baochuan provider.
#[derive(Debug, Clone)]
pub struct AiWiki {
    enabled: bool,
    client: Option<AiClient>,
}

// Simplified AI client that calls baochuan
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AiClient {
    model: String,
}

impl AiWiki {
    /// Create a new AI integration from environment configuration.
    pub fn from_env() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
            .or_else(|_| std::env::var("XAI_API_KEY"))
            .unwrap_or_default();

        if api_key.is_empty() {
            return Self { enabled: false, client: None };
        }

        Self {
            enabled: true,
            client: Some(AiClient {
                model: std::env::var("AI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into()),
            }),
        }
    }

    /// Whether AI features are available.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Summarise a wiki page.
    pub fn summarize(&self, _title: &str, _body: &str) -> String {
        if !self.enabled {
            return "AI features disabled — set OPENAI_API_KEY or ANTHROPIC_API_KEY".into();
        }
        // Stub: real implementation will call baochuan
        if let Some(ref _client) = self.client {
            format!("[AI: summarise]")
        } else {
            String::new()
        }
    }

    /// Ask a question about wiki content.
    pub fn ask(&self, _question: &str, _context: &str) -> String {
        if !self.enabled {
            return "AI features disabled".into();
        }
        format!("[AI: ask]")
    }

    /// Semantic search — find pages related to a query.
    pub fn semantic_search(&self, _query: &str, _pages: &[(&str, &str)]) -> Vec<(String, f64)> {
        if !self.enabled {
            return vec![];
        }
        // Stub: real implementation will use baochuan embeddings
        vec![]
    }

    /// Suggest links between wiki pages based on content.
    pub fn suggest_links(&self, _pages: &[(&str, &str, &str)]) -> Vec<(String, String)> {
        if !self.enabled {
            return vec![];
        }
        // Stub
        vec![]
    }

    /// Generate a new wiki page from a prompt.
    pub fn generate_page(&self, _prompt: &str) -> Result<String> {
        if !self.enabled {
            anyhow::bail!("AI features disabled — set OPENAI_API_KEY or ANTHROPIC_API_KEY");
        }
        // Stub
        Ok(format!("# Generated Page\n\n[AI: generated content]"))
    }
}
