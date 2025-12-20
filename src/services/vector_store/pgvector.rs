use async_trait::async_trait;
use pgvector::Vector;
use sqlx::Row;
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use std::time::Duration;

use super::{CollectionInfo, DEFAULT_EMBEDDING_DIM, VectorStore};
use crate::error::VectorStoreError;
use crate::models::{DocumentChunk, SearchResult, Source, SourceType, Tag, VectorStoreConfig};

pub struct PgVectorBackend {
    pool: PgPool,
    table_name: String,
    collection: String,
    embedding_dim: u64,
}

impl PgVectorBackend {
    pub async fn new(
        config: &VectorStoreConfig,
        embedding_dim: u64,
    ) -> Result<Self, VectorStoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(config.pool_max)
            .acquire_timeout(Duration::from_secs(config.pool_acquire_timeout.into()))
            .connect(&config.url)
            .await
            .map_err(|e| VectorStoreError::ConnectionError(e.to_string()))?;

        let backend = Self {
            pool,
            table_name: config.qualified_table_name(),
            collection: config.collection.clone(),
            embedding_dim,
        };

        backend.check_pgvector_extension().await?;

        if let Some(ref schema) = config.schema {
            backend.ensure_schema(schema).await?;
        }

        Ok(backend)
    }

    pub async fn with_defaults(config: &VectorStoreConfig) -> Result<Self, VectorStoreError> {
        Self::new(config, DEFAULT_EMBEDDING_DIM).await
    }

    async fn check_pgvector_extension(&self) -> Result<(), VectorStoreError> {
        let result: Option<(String,)> =
            sqlx::query_as("SELECT extname FROM pg_extension WHERE extname = 'vector'")
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| VectorStoreError::PostgresError(e.to_string()))?;

        if result.is_none() {
            return Err(VectorStoreError::PgVectorExtensionError(
                "pgvector extension is not installed. Run: CREATE EXTENSION vector;".to_string(),
            ));
        }

        Ok(())
    }

    async fn ensure_schema(&self, schema: &str) -> Result<(), VectorStoreError> {
        let query = format!("CREATE SCHEMA IF NOT EXISTS {}", schema);
        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::PostgresError(e.to_string()))?;
        Ok(())
    }

    fn build_location(
        source_type: SourceType,
        source_location: &str,
        source_url: Option<&str>,
        line_start: Option<u32>,
        line_end: Option<u32>,
    ) -> String {
        if source_type.is_external() {
            source_url.unwrap_or(source_location).to_string()
        } else if let (Some(start), Some(end)) = (line_start, line_end) {
            format!("{}:{}-{}", source_location, start, end)
        } else {
            source_location.to_string()
        }
    }
}

#[async_trait]
impl VectorStore for PgVectorBackend {
    async fn health_check(&self) -> Result<bool, VectorStoreError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| true)
            .map_err(|e| VectorStoreError::ConnectionError(e.to_string()))
    }

    async fn get_collection_info(&self) -> Result<Option<CollectionInfo>, VectorStoreError> {
        let table_exists: Option<(String,)> = sqlx::query_as(
            "SELECT table_name FROM information_schema.tables WHERE table_name = $1",
        )
        .bind(&self.collection)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VectorStoreError::PostgresError(e.to_string()))?;

        if table_exists.is_none() {
            return Ok(None);
        }

        let query = format!("SELECT COUNT(*) as count FROM {}", self.table_name);
        let row: (i64,) = sqlx::query_as(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| VectorStoreError::PostgresError(e.to_string()))?;

        Ok(Some(CollectionInfo {
            points_count: row.0 as u64,
        }))
    }

    async fn create_collection(&self) -> Result<(), VectorStoreError> {
        if self.get_collection_info().await?.is_some() {
            return Ok(());
        }

        let create_table = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                id UUID PRIMARY KEY,
                document_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                embedding vector({}) NOT NULL,
                source_type TEXT NOT NULL,
                source_location TEXT NOT NULL,
                source_url TEXT,
                tags TEXT[] NOT NULL DEFAULT '{{}}',
                checksum TEXT NOT NULL,
                created_at TEXT NOT NULL,
                line_start INTEGER,
                line_end INTEGER
            )
            "#,
            self.table_name, self.embedding_dim
        );

        sqlx::query(&create_table)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::CollectionError(e.to_string()))?;

        let indices = [
            format!(
                "CREATE INDEX IF NOT EXISTS {}_embedding_idx ON {} USING hnsw (embedding vector_cosine_ops)",
                self.collection, self.table_name
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS {}_tags_idx ON {} USING GIN(tags)",
                self.collection, self.table_name
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS {}_source_type_idx ON {} (source_type)",
                self.collection, self.table_name
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS {}_document_id_idx ON {} (document_id)",
                self.collection, self.table_name
            ),
        ];

        for index_sql in &indices {
            sqlx::query(index_sql)
                .execute(&self.pool)
                .await
                .map_err(|e| VectorStoreError::CollectionError(e.to_string()))?;
        }

        Ok(())
    }

    async fn upsert_points(&self, chunks: Vec<DocumentChunk>) -> Result<(), VectorStoreError> {
        if chunks.is_empty() {
            return Ok(());
        }

        let query = format!(
            r#"
            INSERT INTO {} (id, document_id, chunk_index, content, embedding, source_type,
                          source_location, source_url, tags, checksum, created_at, line_start, line_end)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (id) DO UPDATE SET
                document_id = EXCLUDED.document_id,
                chunk_index = EXCLUDED.chunk_index,
                content = EXCLUDED.content,
                embedding = EXCLUDED.embedding,
                source_type = EXCLUDED.source_type,
                source_location = EXCLUDED.source_location,
                source_url = EXCLUDED.source_url,
                tags = EXCLUDED.tags,
                checksum = EXCLUDED.checksum,
                created_at = EXCLUDED.created_at,
                line_start = EXCLUDED.line_start,
                line_end = EXCLUDED.line_end
            "#,
            self.table_name
        );

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| VectorStoreError::UpsertError(e.to_string()))?;

        for chunk in chunks {
            let id = uuid::Uuid::parse_str(&chunk.id)
                .map_err(|e| VectorStoreError::UpsertError(format!("Invalid UUID: {}", e)))?;

            let embedding = Vector::from(chunk.dense_vector);
            let tags: Vec<String> = chunk.tags.iter().map(|t| t.to_payload_string()).collect();

            sqlx::query(&query)
                .bind(id)
                .bind(&chunk.document_id)
                .bind(chunk.chunk_index as i32)
                .bind(&chunk.content)
                .bind(&embedding)
                .bind(chunk.source.source_type.to_string())
                .bind(&chunk.source.location)
                .bind(&chunk.source.url)
                .bind(&tags)
                .bind(&chunk.checksum)
                .bind(&chunk.created_at)
                .bind(chunk.line_start.map(|v| v as i32))
                .bind(chunk.line_end.map(|v| v as i32))
                .execute(&mut *tx)
                .await
                .map_err(|e| VectorStoreError::UpsertError(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| VectorStoreError::UpsertError(e.to_string()))?;

        Ok(())
    }

    async fn search(
        &self,
        query_vector: Vec<f32>,
        limit: u64,
        tags: &[Tag],
        source_types: &[SourceType],
        min_score: Option<f32>,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        let embedding = Vector::from(query_vector);

        let mut where_parts = Vec::new();
        let mut param_index = 2;

        for _ in tags {
            where_parts.push(format!("${} = ANY(tags)", param_index));
            param_index += 1;
        }

        if !source_types.is_empty() {
            let placeholders: Vec<String> = source_types
                .iter()
                .map(|_| {
                    let p = format!("${}", param_index);
                    param_index += 1;
                    p
                })
                .collect();
            where_parts.push(format!("source_type IN ({})", placeholders.join(", ")));
        }

        if let Some(score) = min_score {
            where_parts.push(format!("(1 - (embedding <=> $1)) >= {}", score));
        }

        let where_clause = if where_parts.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_parts.join(" AND "))
        };

        let query = format!(
            r#"
            SELECT
                id::text as chunk_id,
                1 - (embedding <=> $1) as score,
                content,
                source_type,
                source_location,
                source_url,
                tags,
                line_start,
                line_end
            FROM {}
            {}
            ORDER BY embedding <=> $1
            LIMIT {}
            "#,
            self.table_name, where_clause, limit
        );

        let mut query_builder = sqlx::query(&query).bind(&embedding);

        for tag in tags {
            query_builder = query_builder.bind(tag.to_payload_string());
        }

        for source_type in source_types {
            query_builder = query_builder.bind(source_type.to_string());
        }

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| VectorStoreError::SearchError(e.to_string()))?;

        let results = rows
            .into_iter()
            .map(|row: PgRow| {
                let chunk_id: String = row.get("chunk_id");
                let score: f64 = row.get("score");
                let content: String = row.get("content");
                let source_type_str: String = row.get("source_type");
                let source_location: String = row.get("source_location");
                let source_url: Option<String> = row.get("source_url");
                let tag_strings: Vec<String> = row.get("tags");
                let line_start: Option<i32> = row.get("line_start");
                let line_end: Option<i32> = row.get("line_end");

                let source_type: SourceType = source_type_str.parse().unwrap_or(SourceType::Local);
                let tags: Vec<Tag> = tag_strings
                    .into_iter()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                let line_start_u32 = line_start.map(|v| v as u32);
                let line_end_u32 = line_end.map(|v| v as u32);

                let location = Self::build_location(
                    source_type,
                    &source_location,
                    source_url.as_deref(),
                    line_start_u32,
                    line_end_u32,
                );

                SearchResult {
                    chunk_id,
                    score: score as f32,
                    content,
                    source: Source {
                        source_type,
                        location: source_location,
                        url: source_url,
                    },
                    tags,
                    location,
                    line_start: line_start_u32,
                    line_end: line_end_u32,
                }
            })
            .collect();

        Ok(results)
    }

    async fn delete_by_tags(&self, tags: &[Tag]) -> Result<(), VectorStoreError> {
        if tags.is_empty() {
            return Ok(());
        }

        let conditions: Vec<String> = tags
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${} = ANY(tags)", i + 1))
            .collect();

        let query = format!(
            "DELETE FROM {} WHERE {}",
            self.table_name,
            conditions.join(" AND ")
        );

        let mut query_builder = sqlx::query(&query);
        for tag in tags {
            query_builder = query_builder.bind(tag.to_payload_string());
        }

        query_builder
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::DeleteError(e.to_string()))?;

        Ok(())
    }

    async fn delete_by_document_ids(
        &self,
        document_ids: &[String],
    ) -> Result<(), VectorStoreError> {
        if document_ids.is_empty() {
            return Ok(());
        }

        let query = format!(
            "DELETE FROM {} WHERE document_id = ANY($1)",
            self.table_name
        );

        sqlx::query(&query)
            .bind(document_ids)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::DeleteError(e.to_string()))?;

        Ok(())
    }

    async fn clear_collection(&self) -> Result<(), VectorStoreError> {
        if self.get_collection_info().await?.is_none() {
            return Ok(());
        }

        let query = format!("TRUNCATE TABLE {}", self.table_name);
        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::DeleteError(e.to_string()))?;

        Ok(())
    }

    async fn delete_by_source_type(&self, source_type: SourceType) -> Result<(), VectorStoreError> {
        let source_type_str = source_type.to_string();
        let source_tag = format!("source:{}", source_type_str);

        let query = format!(
            "DELETE FROM {} WHERE source_type = $1 OR $2 = ANY(tags)",
            self.table_name
        );

        sqlx::query(&query)
            .bind(&source_type_str)
            .bind(&source_tag)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::DeleteError(e.to_string()))?;

        Ok(())
    }

    async fn list_all_tags(&self) -> Result<Vec<(String, u64)>, VectorStoreError> {
        let query = format!(
            r#"
            SELECT tag, COUNT(*) as count
            FROM {}, unnest(tags) as tag
            GROUP BY tag
            ORDER BY count DESC, tag ASC
            "#,
            self.table_name
        );

        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| VectorStoreError::SearchError(e.to_string()))?;

        let tags = rows
            .into_iter()
            .map(|row: PgRow| {
                let tag: String = row.get("tag");
                let count: i64 = row.get("count");
                (tag, count as u64)
            })
            .collect();

        Ok(tags)
    }

    fn collection(&self) -> &str {
        &self.collection
    }
}
