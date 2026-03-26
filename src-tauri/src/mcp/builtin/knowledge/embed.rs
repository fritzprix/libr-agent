use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Mutex;
use std::sync::OnceLock;

static EMBEDDING_MODEL: OnceLock<Mutex<TextEmbedding>> = OnceLock::new();

/// Get or initialize the embedding model.
/// We use a Mutex because fastembed's TextEmbedding requires mutability for some operations,
/// or just to be safe across threads. Actually TextEmbedding in fastembed is thread-safe (Send + Sync).
pub fn get_embedding_model() -> Result<&'static Mutex<TextEmbedding>> {
    if let Some(model) = EMBEDDING_MODEL.get() {
        return Ok(model);
    }

    // Initialize the model. This will download it on first run.
    log::info!("Initializing fastembed model (AllMiniLML6V2)...");
    let model = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
    )?;

    EMBEDDING_MODEL
        .set(Mutex::new(model))
        .map_err(|_| anyhow::anyhow!("Failed to set embedding model"))?;
    Ok(EMBEDDING_MODEL.get().unwrap())
}

/// Generate an embedding for a single text chunk.
pub fn generate_embedding(text: &str) -> Result<Vec<f32>> {
    let model_mutex = get_embedding_model()?;
    let mut model = model_mutex.lock().unwrap();

    // generate expects a vector of strings
    let embeddings = model.embed(vec![text], None)?;

    if let Some(first) = embeddings.into_iter().next() {
        Ok(first)
    } else {
        Err(anyhow::anyhow!("No embedding generated"))
    }
}
