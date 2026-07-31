#!/usr/bin/env python3
import os
import sys
import json
import argparse
import subprocess
import yaml

def check_hardware():
    print("🔍 [Pre-flight] Checking hardware configuration...")
    try:
        import torch
        if not torch.cuda.is_available():
            print("⚠️ WARNING: CUDA GPU not detected. PyTorch reports CUDA is unavailable.")
            return {"cuda": False, "vram_gb": 0, "device_name": "CPU only"}
        
        device_id = torch.cuda.current_device()
        device_name = torch.cuda.get_device_name(device_id)
        total_memory = torch.cuda.get_device_properties(device_id).total_memory
        vram_gb = total_memory / (1024 ** 3)
        
        print(f"✅ GPU Detected: {device_name}")
        print(f"📊 Total VRAM: {vram_gb:.2f} GB")
        
        return {"cuda": True, "vram_gb": vram_gb, "device_name": device_name}
    except ImportError:
        print("⚠️ WARNING: torch package is not installed. Cannot verify CUDA GPU. Falling back to CPU checks.")
        return {"cuda": False, "vram_gb": 0, "device_name": "Unknown"}

def build_llama_factory_config(data_path, output_dir, hardware_info):
    print("🛠️ Generating training configuration...")
    vram = hardware_info["vram_gb"]
    
    # Optimize hyperparameters dynamically based on available VRAM
    if vram < 12.0:
        print("💡 VRAM < 12GB: Selecting ultra-low memory configurations (QLoRA, BS=1, Accum=8)")
        batch_size = 1
        grad_accumulation = 8
        lora_rank = 8
        quantization = "4bit"
    elif vram < 16.0:
        print("💡 VRAM 12GB-16GB: Selecting low-memory configurations (LoRA, BS=2, Accum=4)")
        batch_size = 2
        grad_accumulation = 4
        lora_rank = 16
        quantization = "none"
    else:
        print("🚀 VRAM >= 16GB: Selecting optimal memory configurations (LoRA, BS=4, Accum=4)")
        batch_size = 4
        grad_accumulation = 4
        lora_rank = 16
        quantization = "none"

    config = {
        "stage": "sft",
        "do_train": True,
        "model_name_or_path": "Qwen/Qwen2.5-7B-Instruct",  # Default base model
        "dataset": "libragent_dataset",
        "dataset_dir": os.path.dirname(data_path),
        "template": "qwen",
        "finetuning_type": "lora",
        "lora_target": "all",
        "lora_rank": lora_rank,
        "output_dir": output_dir,
        "overwrite_output_dir": True,
        "cutoff_len": 1024,
        "preprocessing_num_workers": 4,
        "per_device_train_batch_size": batch_size,
        "gradient_accumulation_steps": grad_accumulation,
        "lr_scheduler_type": "cosine",
        "logging_steps": 10,
        "save_steps": 100,
        "learning_rate": 2e-4,
        "num_train_epochs": 3.0,
        "plot_loss": True,
        "fp16": True if hardware_info["cuda"] else False,
    }

    if quantization == "4bit" and hardware_info["cuda"]:
        config["quantization_bit"] = 4
        config["double_quant"] = True
        config["quantization_type"] = "nf4"

    # Register dataset in Llama-Factory's dataset_info.json format
    dataset_info = {
        "libragent_dataset": {
            "file_name": os.path.basename(data_path),
            "formatting": "sharegpt",
            "columns": {
                "messages": "conversations"
            }
        }
    }

    dataset_info_path = os.path.join(os.path.dirname(data_path), "dataset_info.json")
    with open(dataset_info_path, "w") as f:
        json.dump(dataset_info, f, indent=2)

    config_path = os.path.join(output_dir, "llama_factory_config.yaml")
    os.makedirs(output_dir, exist_ok=True)
    with open(config_path, "w") as f:
        yaml.safe_dump(config, f, default_flow_style=False)

    return config_path

def run_training(config_path):
    print("🔥 Starting fine-tuning using Llama-Factory...")
    cmd = ["llamafactory-cli", "train", config_path]
    print(f"Running command: {' '.join(cmd)}")
    
    try:
        process = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        for line in process.stdout:
            print(line, end="")
        process.wait()
        if process.returncode == 0:
            print("🎉 Training completed successfully!")
        else:
            print(f"❌ Training failed with exit code: {process.returncode}")
            sys.exit(process.returncode)
    except FileNotFoundError:
        print("❌ ERROR: 'llamafactory-cli' command not found. Please install Llama-Factory: pip install llamafactory[metrics]")
        sys.exit(1)

def main():
    parser = argparse.ArgumentParser(description="LibrAgent Fine-Tuning Helper")
    parser.add_argument("--data_path", required=True, help="Path to the exported chat JSON file")
    parser.add_argument("--output_dir", required=True, help="Directory to save the trained model weights")
    args = parser.parse_args()

    hardware = check_hardware()
    config_path = build_llama_factory_config(args.data_path, args.output_dir, hardware)
    run_training(config_path)

if __name__ == "__main__":
    main()
