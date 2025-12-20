//! Qdrant vector store backend implementation.

use async_trait::async_trait;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter,
    PayloadIncludeSelector, PointStruct, ScrollPointsBuilder, SearchPointsBuilder,
    UpsertPointsBuilder, VectorParamsBuilder,
};
use std::collections::HashMap;

use super::{CollectionInfo, DEFAULT_EMBEDDING_DIM, VectorStore};
use crate::error::VectorStoreError;
use crate::models::{DocumentChunk, SearchResult, Source, SourceType, Tag, VectorStoreConfig};

/// Qdrant vector store backend.
pub struct QdrantBackend {
    client: Qdrant,
    collection: String,
    embedding_dim: u64,
}

impl QdrantBackend {
    /// Create a new Qdrant backend from configuration with custom embedding dimension.
    pub fn new(config: &VectorStoreConfig, embedding_dim: u64) -> Result<Self, VectorStoreError> {
        let mut builder = Qdrant::from_url(&config.url);

        if let Some(ref api_key) = config.api_key {
            builder = builder.api_key(api_key.clone());
        }

        let client = builder
            .build()
            .map_err(|e| VectorStoreError::ConnectionError(e.to_string()))?;

        Ok(Self {
            client,
            collection: config.collection.clone(),
            embedding_dim,
        })
    }

    /// Create a backend with default configuration.
    pub fn with_defaults() -> Result<Self, VectorStoreError> {
        Self::new(&VectorStoreConfig::default(), DEFAULT_EMBEDDING_DIM)
    }

    fn build_search_filter(tags: &[Tag], source_types: &[SourceType]) -> Option<Filter> {
        let mut must_conditions: Vec<Condition> = Vec::new();

        for tag in tags {
            must_conditions.push(Condition::matches("tags", tag.to_payload_string()));
        }

        if !source_types.is_empty() {
            let source_conditions: Vec<Condition> = source_types
                .iter()
                .map(|st| Condition::matches("source_type", st.to_string()))
                .collect();
            must_conditions.push(Filter::should(source_conditions).into());
        }

        if must_conditions.is_empty() {
            None
        } else {
            Some(Filter::must(must_conditions))
        }
    }
}

#[async_trait]
impl VectorStore for QdrantBackend {
    async fn health_check(&self) -> Result<bool, VectorStoreError> {
        self.client
            .health_check()
            .await
            .map(|_| true)
            .map_err(|e| VectorStoreError::ConnectionError(e.to_string()))
    }

    async fn get_collection_info(&self) -> Result<Option<CollectionInfo>, VectorStoreError> {
        match self.client.collection_info(&self.collection).await {
            Ok(info) => Ok(Some(CollectionInfo {
                points_count: info.result.map_or(0, |r| r.points_count.unwrap_or(0)),
            })),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("not found") || msg.contains("doesn't exist") {
                    Ok(None)
                } else {
                    Err(VectorStoreError::CollectionError(msg))
                }
            }
        }
    }

    async fn create_collection(&self) -> Result<(), VectorStoreError> {
        if self.get_collection_info().await?.is_some() {
            return Ok(());
        }

        let create_collection = CreateCollectionBuilder::new(&self.collection).vectors_config(
            VectorParamsBuilder::new(self.embedding_dim, Distance::Cosine),
        );

        self.client
            .create_collection(create_collection)
            .await
            .map_err(|e| VectorStoreError::CollectionError(e.to_string()))?;

        Ok(())
    }

    async fn upsert_points(&self, chunks: Vec<DocumentChunk>) -> Result<(), VectorStoreError> {
        if chunks.is_empty() {
            return Ok(());
        }

        let points: Vec<PointStruct> = chunks
            .into_iter()
            .map(|chunk| {
                let mut payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();
                payload.insert("document_id".to_string(), chunk.document_id.into());
                payload.insert(
                    "chunk_index".to_string(),
                    i64::from(chunk.chunk_index).into(),
                );
                payload.insert("content".to_string(), chunk.content.into());
                payload.insert(
                    "source_type".to_string(),
                    chunk.source.source_type.to_string().into(),
                );
                payload.insert("source_location".to_string(), chunk.source.location.into());
                if let Some(url) = chunk.source.url {
                    payload.insert("source_url".to_string(), url.into());
                }
                payload.insert("checksum".to_string(), chunk.checksum.into());
                payload.insert("created_at".to_string(), chunk.created_at.into());

                let tag_strings: Vec<qdrant_client::qdrant::Value> = chunk
                    .tags
                    .into_iter()
                    .map(|t| t.to_payload_string().into())
                    .collect();
                payload.insert("tags".to_string(), tag_strings.into());

                if let Some(line_start) = chunk.line_start {
                    payload.insert("line_start".to_string(), i64::from(line_start).into());
                }
                if let Some(line_end) = chunk.line_end {
                    payload.insert("line_end".to_string(), i64::from(line_end).into());
                }

                PointStruct::new(chunk.id, chunk.dense_vector, payload)
            })
            .collect();

        let upsert = UpsertPointsBuilder::new(&self.collection, points);

        self.client
            .upsert_points(upsert)
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
        let filter = Self::build_search_filter(tags, source_types);

        let mut search_builder =
            SearchPointsBuilder::new(&self.collection, query_vector, limit).with_payload(true);

        if let Some(f) = filter {
            search_builder = search_builder.filter(f);
        }

        if let Some(score) = min_score {
            search_builder = search_builder.score_threshold(score);
        }

        let results = self
            .client
            .search_points(search_builder)
            .await
            .map_err(|e| VectorStoreError::SearchError(e.to_string()))?;

        let search_results: Vec<SearchResult> = results
            .result
            .into_iter()
            .map(|point| {
                let payload = point.payload;

                let content = payload
                    .get("content")
                    .and_then(|v| match &v.kind {
                        Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => {
                            Some(s.as_str())
                        }
                        _ => None,
                    })
                    .unwrap_or("")
                    .to_string();

                let source_type_str = payload
                    .get("source_type")
                    .and_then(|v| match &v.kind {
                        Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => {
                            Some(s.as_str())
                        }
                        _ => None,
                    })
                    .unwrap_or("local");
                let source_type: SourceType = source_type_str.parse().unwrap_or(SourceType::Local);

                let source_location = payload
                    .get("source_location")
                    .and_then(|v| match &v.kind {
                        Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => {
                            Some(s.as_str())
                        }
                        _ => None,
                    })
                    .unwrap_or("")
                    .to_string();

                let source_url = payload.get("source_url").and_then(|v| match &v.kind {
                    Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => Some(s.clone()),
                    _ => None,
                });

                let tags: Vec<Tag> = payload
                    .get("tags")
                    .and_then(|v| match &v.kind {
                        Some(qdrant_client::qdrant::value::Kind::ListValue(list)) => Some(
                            list.values
                                .iter()
                                .filter_map(|v| match &v.kind {
                                    Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => {
                                        s.parse().ok()
                                    }
                                    _ => None,
                                })
                                .collect(),
                        ),
                        _ => None,
                    })
                    .unwrap_or_default();

                let line_start = payload.get("line_start").and_then(|v| match &v.kind {
                    Some(qdrant_client::qdrant::value::Kind::IntegerValue(n)) => Some(*n as u32),
                    _ => None,
                });

                let line_end = payload.get("line_end").and_then(|v| match &v.kind {
                    Some(qdrant_client::qdrant::value::Kind::IntegerValue(n)) => Some(*n as u32),
                    _ => None,
                });

                let location = if source_type.is_external() {
                    source_url
                        .as_deref()
                        .unwrap_or(&source_location)
                        .to_string()
                } else if let (Some(start), Some(end)) = (line_start, line_end) {
                    format!("{}:{}-{}", source_location, start, end)
                } else {
                    source_location.clone()
                };

                let source = Source {
                    source_type,
                    location: source_location,
                    url: source_url,
                };

                let chunk_id = match &point.id {
                    Some(id) => match &id.point_id_options {
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid)) => {
                            uuid.clone()
                        }
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(num)) => {
                            num.to_string()
                        }
                        None => String::new(),
                    },
                    None => String::new(),
                };

                SearchResult {
                    chunk_id,
                    score: point.score,
                    content,
                    source,
                    tags,
                    location,
                    line_start,
                    line_end,
                }
            })
            .collect();

        Ok(search_results)
    }

    async fn delete_by_tags(&self, tags: &[Tag]) -> Result<(), VectorStoreError> {
        if tags.is_empty() {
            return Ok(());
        }

        let filter_conditions: Vec<Condition> = tags
            .iter()
            .map(|tag| Condition::matches("tags", tag.to_payload_string()))
            .collect();

        let filter = Filter::must(filter_conditions);
        let delete = DeletePointsBuilder::new(&self.collection).points(filter);

        self.client
            .delete_points(delete)
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

        let conditions: Vec<Condition> = document_ids
            .iter()
            .map(|id| Condition::matches("document_id", id.clone()))
            .collect();

        let filter = Filter::should(conditions);
        let delete = DeletePointsBuilder::new(&self.collection).points(filter);

        self.client
            .delete_points(delete)
            .await
            .map_err(|e| VectorStoreError::DeleteError(e.to_string()))?;

        Ok(())
    }

    async fn clear_collection(&self) -> Result<(), VectorStoreError> {
        if self.get_collection_info().await?.is_none() {
            return Ok(());
        }

        self.client
            .delete_collection(&self.collection)
            .await
            .map_err(|e| VectorStoreError::DeleteError(e.to_string()))?;

        self.create_collection().await?;

        Ok(())
    }

    async fn delete_by_source_type(&self, source_type: SourceType) -> Result<(), VectorStoreError> {
        let source_type_str = source_type.to_string();
        let source_tag = format!("source:{}", source_type_str);

        let filter = Filter::should([
            Condition::matches("source_type", source_type_str),
            Condition::matches("tags", source_tag),
        ]);
        let delete = DeletePointsBuilder::new(&self.collection).points(filter);

        self.client
            .delete_points(delete)
            .await
            .map_err(|e| VectorStoreError::DeleteError(e.to_string()))?;

        Ok(())
    }

    async fn list_all_tags(&self) -> Result<Vec<(String, u64)>, VectorStoreError> {
        let mut tag_counts: HashMap<String, u64> = HashMap::new();
        let mut offset: Option<qdrant_client::qdrant::PointId> = None;
        let batch_size = 100u32;

        loop {
            let mut scroll_builder = ScrollPointsBuilder::new(&self.collection)
                .limit(batch_size)
                .with_payload(PayloadIncludeSelector {
                    fields: vec!["tags".to_string()],
                })
                .with_vectors(false);

            if let Some(off) = offset {
                scroll_builder = scroll_builder.offset(off);
            }

            let response = self
                .client
                .scroll(scroll_builder)
                .await
                .map_err(|e| VectorStoreError::SearchError(e.to_string()))?;

            let points = response.result;
            if points.is_empty() {
                break;
            }

            for point in &points {
                let Some(tags_value) = point.payload.get("tags") else {
                    continue;
                };
                let Some(qdrant_client::qdrant::value::Kind::ListValue(list)) = &tags_value.kind
                else {
                    continue;
                };
                for v in &list.values {
                    if let Some(qdrant_client::qdrant::value::Kind::StringValue(tag_str)) = &v.kind
                    {
                        *tag_counts.entry(tag_str.clone()).or_insert(0) += 1;
                    }
                }
            }

            offset = response.next_page_offset;
            if offset.is_none() {
                break;
            }
        }

        let mut tags: Vec<(String, u64)> = tag_counts.into_iter().collect();
        tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Ok(tags)
    }

    fn collection(&self) -> &str {
        &self.collection
    }
}
