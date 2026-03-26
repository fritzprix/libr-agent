use anyhow::{anyhow, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static EMBEDDING_MODEL: OnceLock<Mutex<TextEmbedding>> = OnceLock::new();
static EMBEDDING_RUNTIME: OnceLock<EmbeddingRuntime> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct EmbeddingRuntime {
    pub model_name: &'static str,
    pub cache_dir: PathBuf,
    pub provider_strategy: &'static str,
    pub show_download_progress: bool,
}

pub fn runtime_details() -> &'static EmbeddingRuntime {
    EMBEDDING_RUNTIME.get_or_init(|| EmbeddingRuntime {
        model_name: "AllMiniLML6V2",
        cache_dir: default_cache_dir(),
        provider_strategy: if cfg!(target_os = "windows") {
            "ONNX Runtime default provider selection (DirectML is not forced in the current build)"
        } else {
            "ONNX Runtime default provider selection"
        },
        show_download_progress: true,
    })
}

pub fn runtime_summary() -> String {
    let runtime = runtime_details();
    format!(
        "Embedding model {} using {}. Cache directory: {}",
        runtime.model_name,
        runtime.provider_strategy,
        runtime.cache_dir.display()
    )
}

/// Get or initialize the embedding model.
/// We use a Mutex because fastembed's TextEmbedding requires mutability for some operations,
/// or just to be safe across threads. Actually TextEmbedding in fastembed is thread-safe (Send + Sync).
pub fn get_embedding_model() -> Result<&'static Mutex<TextEmbedding>> {
    if let Some(model) = EMBEDDING_MODEL.get() {
        return Ok(model);
    }

    let runtime = runtime_details();
    ensure_cache_dir(&runtime.cache_dir)?;

    log::info!(
        "Initializing fastembed model ({}) with cache dir {} and provider strategy: {}",
        runtime.model_name,
        runtime.cache_dir.display(),
        runtime.provider_strategy
    );
    let model = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::AllMiniLML6V2)
            .with_cache_dir(runtime.cache_dir.clone())
            .with_show_download_progress(runtime.show_download_progress),
    )?;

    EMBEDDING_MODEL
        .set(Mutex::new(model))
        .map_err(|_| anyhow!("Failed to set embedding model"))?;
    EMBEDDING_MODEL
        .get()
        .ok_or_else(|| anyhow!("Embedding model was not initialized"))
}

/// Generate an embedding for a single text chunk.
pub fn generate_embedding(text: &str) -> Result<Vec<f32>> {
    let model_mutex = get_embedding_model()?;
    let mut model = model_mutex
        .lock()
        .map_err(|_| anyhow!("Embedding model mutex was poisoned"))?;

    // generate expects a vector of strings
    let embeddings = model.embed(vec![text], None)?;

    if let Some(first) = embeddings.into_iter().next() {
        Ok(first)
    } else {
        Err(anyhow!("No embedding generated"))
    }
}

fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("libragent")
        .join("fastembed")
}

fn ensure_cache_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|error| {
        anyhow!(
            "Failed to create embedding cache dir {}: {}",
            path.display(),
            error
        )
    })
}
