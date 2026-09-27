# ComfyUI workflows for dream images

API-format ComfyUI graphs the runtime fills in and submits (`POST /prompt`) when the agent dreams.
Placeholders: `{{prompt}}`, `{{negative}}`, `{{seed}}` (an integer; the quotes are replaced), `{{prefix}}` (output filename prefix).

| File | Model | Notes |
|---|---|---|
| `qwen_image_2_1.api.json` | Qwen Image 2.1 (int8) + qwen3vl-8b text encoder | Primary. 25 steps, 832×1248 (2:3, ~1 MP). Renders legible text if asked, so the visual prompt must not contain prose; keep "text, letters, writing" in the negative. |
| `sdxl_turbo.api.json` | SDXL Turbo | Fallback. 4 steps, a few seconds; cannot render text. |

Model filenames must match what `GET /models/{diffusion_models,text_encoders,vae,checkpoints}` lists on your ComfyUI. Only core nodes are used.
