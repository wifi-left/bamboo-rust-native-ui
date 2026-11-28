# Bamboo Native UI documentation

A small, declarative native Windows UI framework built for Bamboo Music Player.
GDI + dirty-rectangle rendering, backend-abstracted, no egui/wgpu/WebView.

## Languages

| Language | Contents |
|---|---|
| **English** | [`en/index.md`](en/index.md) — overview, thirteen quick starts, API reference, 62 widget pages, guides, lookup tables. |
| **中文** | [`zh/index.md`](zh/index.md) — the same structure, translated. |

## Machine-readable entry points

- [`llms.txt`](llms.txt) — curated index following the llmstxt.org convention.
- [`en/ai/reference.md`](en/ai/reference.md) — one self-contained context file
  (crate map, feature flags, core signatures, widget catalog, XML and theme
  grammar, gotchas) for AI assistants.
- [`en/ai/widgets.md`](en/ai/widgets.md) — compact widget lookup table.

## Building the HTML site

The site is built with [mkdocs-material](https://squidfunk.github.io/mkdocs-material/):

```powershell
pip install -r requirements-docs.txt
mkdocs build            # English -> site/
mkdocs build -f mkdocs.zh.yml   # Chinese -> site/zh/
```

The GitHub Actions workflow in `.github/workflows/docs.yml` builds both and
publishes the result to the `gh-pages` branch on every push that touches the
documentation.

## About these documents

- `DOC-GUIDE.md` — the authoring contract (templates, terminology, link rules).
- `archive/` — older design records and milestone contracts kept for
  reference: `API.md`, `DECISIONS.md`, `M7-XML.md`, `CUSTOM-WIDGETS.md`.
- `HANDOFF.md` and `TODO.md` are local working notes and are not tracked.
