---
title: shivvr and grub — state, contracts, PDF path
status: proposed
source: grubcrawler-76 session (read from shivvr src/api.rs and src/embed_api.rs at 7ed0120; grub behavior tested), Claude (live /invert check), for Kord
date: 2026-09-27
related: docs/EMBEDDINGS_PLAN.md, docs/BACKLOG.md (O4, L3, X4), inbox/WORKPLAN.md
---

# shivvr and grub

## State
- **shivvr:**
  - Branch `feat/vision-audio-embed` (`57c2516` fleet WIP: SigLIP, audio, gateway; `7ed0120` `POST /embed`) has **no upstream and is unpushed**. Local `main` is one commit ahead of `origin/main`.
  - `Cargo.toml` says 0.3.0; the only tag is v0.2.0. There's no CI release pipeline (Cloud Build to GCR, Cloud Run).
  - The `:8085` container was built from that HEAD, so the endpoints below match the source.
- **grub:**
  - v0.14.0 (browser-hang fix).
  - **No PDF support.** `/api/crawl` and `/api/markdown` on a PDF scrape Firefox's pdf.js viewer: a 15-page paper returned 693 words, title and abstract only. `GET /download?url=` returns the raw bytes correctly.
- **Ferricula's own Rust PDF extraction works** (the benchmark PDF: 15 pages, 28 sections) and stays the path for PDFs it fetches directly.
- **Old OCR path:** `Gnosis/gnosis-ocr` (poppler renders pages, a vision model does OCR, pages joined as "## Page N") is stale. Salvage its page loop and prompt; don't call it.

## Contracts Ferricula uses (as implemented)
| Route | Request | Response | Limits and errors |
|---|---|---|---|
| `POST /embed` | `{texts: [str], model?: "gtr-t5-base"}` | `{model, dim: 768, vectors}` (mean-pooled, L2-normalised GTR-T5-base) | 1–256 texts, each non-blank and ≤ 32,768 bytes; any other model is rejected; `{error}` with a readable message; no timeout contract |
| `POST /image/embed` | `{image_base64}` | `{embedding[768], dimension, model: "siglip-base-patch16-224"}` | 503 if vision isn't loaded; 400 bad base64; **2 MB body limit** (axum default, no `DefaultBodyLimit` set) |
| `POST /invert` | `{embedding, role?, max_length?: 64}` | `{text, similarity}` | 503 if not loaded. **Checked live on 2026-09-27:** a GTR vector of the eulogy line inverted to a close paraphrase at similarity 0.88, so it works in our space |
| `POST /audio/transcribe`, `/audio/embed` | `{audio_base64}` | `{transcript}` / `{transcript, embedding[768 GTR]}` | forwarded to `TRANSCRIPTION_URL` (the hyperia-transcription container); 60 s timeout |
| `GET /health` | — | version, loaded models, `inversion_available`, `audio_available` | check before calling vision or inversion |

**Auth:** the nuts-auth gate is installed only when `NUTS_AUTH_JWKS_URL` is set. Local stays open; send an `ahp_` token only to `shivvr.nuts.services`.

## Recommendation on PDFs (grubcrawler session)
- **Extraction belongs in grub** (Python; PyMuPDF is one dependency; it already fetches bytes and has a vision path). grub v0.15.0 would:
  - detect `application/pdf`;
  - use the text layer first, with page-render plus vision OCR for image-only pages (the gnosis-ocr prompt);
  - add a page-render endpoint returning per-page PNG (base64), page numbers and per-page text.
- **Ferricula** keeps its Rust extraction, calls grub for scanned PDFs and PDFs found while crawling, and sends rendered pages to shivvr `/image/embed` itself (backlog L3, PDF-page SigLIP).
- **shivvr** stays text, image and audio in; no PDF rendering there.

## Still needed
| Item | Where | Notes |
|---|---|---|
| Push the branch, merge to main, tag v0.3.0, add a release pipeline | shivvr | **waiting on Kord's go**; backlog O4 |
| `build.rs`: four hardcoded Visual Studio `lib.exe` paths (`build.rs:53-56`, fallback `:69`) | shivvr | vswhere or an env override; doesn't block Docker/Linux builds |
| Per-route body limit for `/image/embed` | shivvr | rendered PDF pages at real DPI exceed 2 MB → 413 |
| SigLIP **text** tower (`models/siglip-text.onnx` ships but isn't loaded) | shivvr | needed for text↔image comparison; `/embed` rejects `model=siglip…` by design |
| Server-side request timeouts | shivvr | embedding runs synchronously in handlers |
| **Security:** the MCP tool list includes `run_command` (shell inside the container) | shivvr | Kord to decide: remove or gate it |
| PDF support (v0.15.0 above) | grub | grubcrawler session offers to build it |

## Who builds what
The grubcrawler session takes the grub side on Kord's go, and offers to take the shivvr items if assigned. The Ferricula side (grub for scanned PDFs, page images to SigLIP, ghost echoes via `/invert`) is backlog L3 and joins the work plan once the grub and shivvr pieces exist.

## Update, 2026-09-27 evening (grubcrawler session): K7 carried out
- **shivvr v0.4.0** (`main` `6dd03d6`, tags v0.3.0 and v0.4.0 pushed):
  - Body limits: 32 MiB on `/image/embed`, `/audio/*` and ingest; 8 MiB on `/embed`; 2 MiB elsewhere.
  - Timeouts: 120 s on inference routes, 30 s elsewhere (`SHIVVR_REQUEST_TIMEOUT_SECS`); 408 with `{error}` when exceeded.
  - **SigLIP text tower:** `POST /embed {"model": "siglip-base-patch16-224"}` returns 768-d vectors in the `/image/embed` space. It needs `models/siglip-tokenizer.json`, which only exists after a models-image rebuild (`deploy.sh --rebuild-models`). Until then it returns 503; GTR-T5 is unchanged.
  - **`run_command` is off** unless `SHIVVR_ENABLE_RUN_COMMAND=true`. Ferricula never used it.
  - Gateway image `ghcr.io/deepbluedynamics/shivvr-gateway:0.3.0` is published; 0.4.0 is building.
- **grub v0.15.0** (tagged, building on Docker Hub):
  - PDF URLs on `/api/crawl`, `/api/markdown` and `/api/batch` return per-page markdown (`## Page N`). Vision OCR handles image-only pages when the container has a provider key; the local one doesn't yet. `render_mode` is one of `pdf_text | pdf_vision | pdf_mixed | pdf_empty`.
  - `POST /api/pdf/pages {url, pages?, dpi 36–300, max_pages ≤100, include_text, include_images}` returns `page_count`, `title` and `pages[]`, each with `text`, `char_count`, `image_base64` (PNG), `width` and `height`. Page images go straight to shivvr `/image/embed`.
  - MCP/AHP tools added: `download`, `pdf_extract`.
  - On the 15-page test PDF: 6,453 words in 7 s, against 693 before.
- **Still manual (Kord):**
  - Redeploy local shivvr: `:8085` still runs v0.3.0.
  - Redeploy local grub to v0.15.0.
  - Rebuild the shivvr models image for the SigLIP tokenizer.
  - Cloud Build / Cloud Run deploys for `shivvr.nuts.services`.
- **Ferricula follow-ups (backlog L3 and a new E1):**
  - E1: use grub `/api/markdown` for PDFs whose text layer is empty (`render_mode` `pdf_vision`/`pdf_empty` would tell us), keeping the Rust extraction as the first path.
  - L3: page images from `/api/pdf/pages` → `/image/embed` → seeing rows; SigLIP text-tower queries against them once the models image is rebuilt.

## grub OCR via Ollama (grubcrawler session, 2026-09-27, in progress)
- **Model:** `benhaotang/Nanonets-OCR-s` (already on the host) via `/api/chat`, `images`, `stream:false`.
  - Options: temperature 0.01, `num_predict` 4096, `num_ctx` 8192.
  - `keep_alive` 5m within a document, unloaded (`keep_alive: 0`) after the last OCR page, to give the shared RTX 3060 back.
  - Concurrency 1; long side capped at 1280 px; 180 s per page; retry once on an empty page, then mark it failed; repetition cutoff.
- **Env:** `OLLAMA_BASE_URL` (default `http://host.docker.internal:11434`), `OLLAMA_VISION_MODEL`, and `OLLAMA_API_KEY` for a hosted Ollama on Cloud Run.
- **Measured** from the grub container (110 DPI letter page, ~909×1287 px): cold 22.8 s (8.6 s model load), warm 2.0 s, clean markdown including an HTML table.
- **Contract for Ferricula (E1):**
  - Per-page `source`: `text_layer | ocr | empty | error`, with `ocr_model` on OCR pages, in `/api/pdf/pages` and in crawl metadata (`page_info.pdf.pages`).
  - The markdown carries `<!-- ocr: <model> -->` under each OCR'd page heading.
  - Ferricula labels those sections as **transcribed, not verbatim**, so the agent doesn't quote OCR errors as the author's words.
  - `render_mode` stays `pdf_text | pdf_vision | pdf_mixed | pdf_empty`.
- The grub version tag will follow when it ships.

## Deploy status (grubcrawler session, 2026-09-27 evening)
- **Production:**
  - shivvr v0.4.2 on the GPU backend (revision `shivvr-00015-2qm`); gateway `shivvr.nuts.services` v0.4.1+.
  - grub v0.16.0 on Cloud Run and Docker Hub, with the Ollama OCR path. Locally OCR uses Nanonets via host Ollama; on Cloud Run it uses Anthropic with Kord's key.
  - Body limits, timeouts, SigLIP vision and `run_command` off by default are all live.
- **Still pending:** the SigLIP **text** tower. `siglip-tokenizer.json` isn't produced yet (transformers 4.44 lacks the converter), so `/embed` with `model=siglip-base-patch16-224` returns 503 until the grubcrawler session builds the tokenizer from `spiece.model` and rebuilds the models image.
- **grub OCR contract as shipped:** per-page `source` is `text_layer | ocr | empty | error`, with `ocr_model`; OCR'd pages carry `<!-- ocr: <model> -->`; `render_mode` and `POST /api/pdf/pages` are as above.
- **Local containers used by Steve are older:** shivvr `:8085` and grub `:6792` (0.13.3). Until Kord redeploys them, Steve's `ingest_url` gets no PDF support or OCR from grub, and local shivvr lacks the new limits.

## SigLIP text tower live (grubcrawler session, 2026-09-27 night)
- **shivvr v0.4.3** in production (revision `shivvr-00016-fbq`) **and locally on `:8085`**.
  - `POST /embed {"model": "siglip-base-patch16-224"}` returns 768-d vectors in the `/image/embed` space.
  - The tokenizer is built from `spiece.model` and checked against the slow tokenizer at export.
- **Rank, don't threshold.** SigLIP cosines are small (the matching caption scored ~0.13, non-matches ~0 or below); rank by similarity.
- **Local `:8085` is now v0.4.3:** 32 MiB media bodies, 8 MiB `/embed`, 120 s / 30 s timeouts with 408, `run_command` off.
- **Checked after the swap** (Claude): Steve's embedding probe is still `ok` (GTR-T5 cosines 1.0 on the three probe memories), so his recall space is unchanged.
