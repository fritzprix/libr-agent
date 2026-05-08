use crate::entity::knowledge_chunk_v2;
use crate::repositories::DbError;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use std::cmp::Ordering;
use std::collections::HashMap;

use super::repository::{build_literal_fts_query, embedding_bytes, row_to_chunk_model};

fn fuse_ranked_rows(
    rows: Vec<sea_orm::QueryResult>,
    fused_scores: &mut HashMap<i32, f32>,
    chunk_models: &mut HashMap<i32, knowledge_chunk_v2::Model>,
) -> Result<(), DbError> {
    const RRF_K: f32 = 60.0;

    for (index, row) in rows.into_iter().enumerate() {
        let model = row_to_chunk_model(&row)?;
        let rank = index as f32 + 1.0;
        *fused_scores.entry(model.id).or_insert(0.0) += 1.0 / (RRF_K + rank);
        chunk_models.entry(model.id).or_insert(model);
    }

    Ok(())
}

async fn run_keyword_search(
    db: &DatabaseConnection,
    assistant_id: &str,
    query: &str,
    limit: u64,
) -> Result<Vec<sea_orm::QueryResult>, DbError> {
    let keyword_sql = r#"
        SELECT
            c.id, c.assistant_id, c.content, c.tags, c.source, c.created_at,
            bm25(knowledge_chunks_fts) AS bm25_rank
        FROM knowledge_chunks_fts
        JOIN knowledge_chunks_v2 c ON c.id = knowledge_chunks_fts.rowid
        WHERE c.assistant_id = ? AND knowledge_chunks_fts MATCH ?
        ORDER BY bm25_rank
        LIMIT ?
    "#;

    db.query_all(Statement::from_sql_and_values(
        db.get_database_backend(),
        keyword_sql,
        [assistant_id.into(), query.into(), limit.into()],
    ))
    .await
    .map_err(DbError::SeaOrmQueryFailed)
}

async fn run_semantic_search(
    db: &DatabaseConnection,
    assistant_id: &str,
    query_embedding: Vec<f32>,
    limit: u64,
) -> Result<Vec<sea_orm::QueryResult>, DbError> {
    let semantic_sql = r#"
        SELECT
            c.id, c.assistant_id, c.content, c.tags, c.source, c.created_at,
            v.distance
        FROM knowledge_chunks_v2 c
        JOIN knowledge_vectors v ON c.id = v.rowid
        WHERE c.assistant_id = ? AND v.embedding MATCH ? AND k = ?
        ORDER BY v.distance
    "#;

    let query_bytes = embedding_bytes(&query_embedding);
    db.query_all(Statement::from_sql_and_values(
        db.get_database_backend(),
        semantic_sql,
        [assistant_id.into(), query_bytes.into(), limit.into()],
    ))
    .await
    .map_err(DbError::SeaOrmQueryFailed)
}

pub(super) async fn search_hybrid(
    db: &DatabaseConnection,
    assistant_id: &str,
    text_query: Option<&str>,
    query_embedding: Option<Vec<f32>>,
    limit: u64,
) -> Result<Vec<(knowledge_chunk_v2::Model, f32)>, DbError> {
    if text_query.is_none() && query_embedding.is_none() {
        return Err(DbError::InvalidInput(
            "search_hybrid requires a text query, an embedding, or both".to_string(),
        ));
    }

    let mut fused_scores = HashMap::<i32, f32>::new();
    let mut chunk_models = HashMap::<i32, knowledge_chunk_v2::Model>::new();

    if let Some(query) = text_query.map(str::trim).and_then(build_literal_fts_query) {
        fuse_ranked_rows(
            run_keyword_search(db, assistant_id, &query, limit).await?,
            &mut fused_scores,
            &mut chunk_models,
        )?;
    }

    if let Some(query_embedding) = query_embedding {
        fuse_ranked_rows(
            run_semantic_search(db, assistant_id, query_embedding, limit).await?,
            &mut fused_scores,
            &mut chunk_models,
        )?;
    }

    let mut ranked_ids = fused_scores.into_iter().collect::<Vec<_>>();
    ranked_ids.sort_by(|(left_id, left_score), (right_id, right_score)| {
        right_score
            .partial_cmp(left_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left_id.cmp(right_id))
    });

    let mut output = Vec::new();
    for (chunk_id, score) in ranked_ids.into_iter().take(limit as usize) {
        if let Some(model) = chunk_models.remove(&chunk_id) {
            output.push((model, score));
        }
    }

    Ok(output)
}
