//! # jaringan-wiki
//!
//! AI-powered private wikis over the Jaringan protocol.
//!
//! ## Architecture
//!
//! A wiki is a directory of markdown files served as signed Jaringan pages.
//! Each file is a wiki page. The wiki server implements the `PageResolver`
//! trait so it can be served directly over JRG TCP or via the HTTP gateway.
//!
//! AI features (using baochuan):
//! - Semantic search across wiki pages
//! - Q&A about wiki content
//! - Auto-summarise pages
//! - Auto-link pages based on content
//! - Generate new pages from prompts

pub mod wiki;
pub mod resolver;
pub mod search;
pub mod ai;

pub use wiki::{Wiki, WikiPage, WikiConfig};
pub use resolver::WikiResolver;
pub use search::SearchIndex;
pub use ai::AiWiki;
