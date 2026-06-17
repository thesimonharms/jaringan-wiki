# jaringan-wiki

**AI-powered private wikis over the Jaringan protocol.**

A wiki server that turns a directory of markdown files into signed, encrypted Jaringan pages — browseable with the [Jaringan Browser](https://github.com/thesimonharms/jaringan). AI features (semantic search, summarisation, Q&A, page generation) via the [baochuan](https://github.com/thesimonharms/baochuan) Rust AI client.

## Quick Start

```bash
# Create a new wiki
jaringan-wiki init ~/my-wiki

# Serve it over Jaringan
jaringan-wiki serve ~/my-wiki

# In another terminal, browse with the Jaringan Browser
jaringan-browser jrg://wiki/
```

## Commands

| Command | Description |
|---------|-------------|
| `init <path>` | Initialise a new wiki directory with sample pages |
| `create <path> <slug>` | Create a new wiki page |
| `serve <path>` | Serve wiki as a JRG TCP server |
| `search <path> <query>` | Full-text search across wiki pages |
| `list <path>` | List all pages in the wiki |
| `generate <path> <prompt>` | AI: generate a page from a description |

## Wiki Format

Pages are plain markdown files with optional YAML front matter:

```markdown
---
title: My Page
tags: [rust, networking]
---

# My Page

Content here. Link to other pages with [[page-slug]]
or [[page-slug|Display Text]].
```

The server converts them to signed Jaringan pages on the fly. Wiki links `[[slug]]` become `jrg://wiki/slug` links.

## AI Features

Set one of these environment variables to enable AI:

| Env var | Provider |
|---------|----------|
| `OPENAI_API_KEY` | OpenAI |
| `ANTHROPIC_API_KEY` | Anthropic Claude |
| `XAI_API_KEY` | xAI Grok |
| `AI_MODEL` | Model override (default: `gpt-4o-mini`) |

Features:
- **Semantic search** — find pages by meaning, not just keywords
- **Page summarisation** — one-line summaries for quick scanning
- **Q&A** — "ask the wiki" about its content
- **Auto-linking** — suggest wiki links between related pages
- **Page generation** — create new pages from a prompt

> *AI methods are stubbed in v0.1 — full baochuan integration coming next.*

## Architecture

```
jaringan-wiki/
├── src/
│   ├── main.rs      # CLI (clap)
│   ├── lib.rs       # Public API
│   ├── wiki.rs      # Content model + markdown parsing
│   ├── resolver.rs  # JRG PageResolver implementation
│   ├── search.rs    # Full-text search index
│   └── ai.rs        # AI integration (baochuan)
├── examples/        # Sample wikis
└── README.md
```

The wiki server implements the Jaringan `PageResolver` trait, so it works with any JRG-compatible client or gateway:

```
JRG Browser ──TCP──→ WikiResolver ──reads──→ *.md files
                    (PageResolver)
```

## License

MIT
