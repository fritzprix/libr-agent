# Hardware Guide (Fine-Tune)

`scripts/train.py` runs pre-flight checks automatically. Use this guide to interpret results and set expectations.

## VRAM tiers

| VRAM | train.py behavior | Expectation |
| --- | --- | --- |
| < 12 GB | QLoRA, batch=1, grad accum=8 | Slow but feasible on consumer GPU |
| 12–16 GB | LoRA, batch=2 | Moderate throughput |
| ≥ 16 GB | LoRA, batch=4 | Preferred for 7B-class models |

## CPU-only fallback

If CUDA unavailable, training may still run but is **impractical** for 7B models. Recommend:

- Smaller base model
- Cloud GPU
- Export dataset only and train elsewhere

## Dependencies

- Python 3.10+
- `torch` with CUDA matching driver (when using GPU)
- Llama-Factory installed per [llama-factory-setup.md](llama-factory-setup.md)

## Privacy pre-check

Before export/training:

- Scan dataset JSON for API keys, tokens, emails
- Use `excludeErrors` and `excludeShort` filters on export
- Redact or drop contaminated sessions

## Disk space

Reserve space for:

- Dataset JSON
- Base model cache (multi-GB)
- Checkpoints under `--output_dir`
