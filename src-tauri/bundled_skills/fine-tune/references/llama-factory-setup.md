# Llama-Factory Setup

`train.py` generates config and invokes Llama-Factory CLI. Prepare the environment once per machine.

## Install (user environment)

```bash
pip install llamafactory
# or follow upstream: https://github.com/hiyouga/LLaMA-Factory
```

Ensure `llamafactory-cli` or the entrypoint used by `train.py` is on PATH.

## Dataset layout

Export via `history__exportDataset` with `format: "llamaFactory"`.

`train.py` expects:

- `--data_path` pointing to exported JSON
- `--output_dir` for checkpoints and logs

Dataset directory is registered in generated config as `libragent_dataset`.

## Training invocation

```bash
python "<skill-base-dir>/scripts/train.py" \
  --data_path workspace/datasets/finetune_data.json \
  --output_dir workspace/models/finetuned_model
```

## After training

1. Locate adapter/full weights under `--output_dir`.
2. Register model with Ollama, llama.cpp, or local HF inference stack.
3. Update assistant model path in LibrAgent settings.

## Troubleshooting

| Error | Check |
| --- | --- |
| CUDA OOM | Re-run; train.py lowers batch for low VRAM |
| Dataset not found | Export path matches `--data_path` |
| CLI not found | Llama-Factory install and PATH |

See [hardware-guide.md](hardware-guide.md) for GPU expectations.
