use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter, PointStruct,
    SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use std::collections::HashMap;

use crate::error::VectorStoreError;
use crate::models::{DocumentChunk, SearchResult, Source, SourceType, Tag, VectorStoreConfig};

pub const EMBEDDING_DIM: u64 = 1024;

pub struct VectorStoreClient {
    client: Qdrant,
    collection: String,
}

impl VectorStoreClient {
    pub async fn new(config: &VectorStoreConfig) -> Result<Self, VectorStoreError> {
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
        })
    }

    pub async fn with_defaults() -> Result<Self, VectorStoreError> {
        Self::new(&VectorStoreConfig::default()).await
    }

    pub async fn health_check(&self) -> Result<bool, VectorStoreError> {
        self.client
            .health_check()
            .await
            .map(|_| true)
            .map_err(|e| VectorStoreError::ConnectionError(e.to_string()))
    }

    pub async fn get_collection_info(&self) -> Result<Option<CollectionInfo>, VectorStoreError> {
        match self.client.collection_info(&self.collection).await {
            Ok(info) => Ok(Some(CollectionInfo {
                points_count: info
                    .result
                    .map(|r| r.points_count.unwrap_or(0))
                    .unwrap_or(0),
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

    pub async fn create_collection(&self) -> Result<(), VectorStoreError> {
        if self.get_collection_info().await?.is_some() {
            return Ok(());
        }

        let create_collection = CreateCollectionBuilder::new(&self.collection)
            .vectors_config(VectorParamsBuilder::new(EMBEDDING_DIM, Distance::Cosine));

        self.client
            .create_collection(create_collection)
            .await
            .map_err(|e| VectorStoreError::CollectionError(e.to_string()))?;

        Ok(())
    }

    pub async fn upsert_points(&self, chunks: Vec<DocumentChunk>) -> Result<(), VectorStoreError> {
        if chunks.is_empty() {
            return Ok(());
        }

        let points: Vec<PointStruct> = chunks
            .into_iter()
            .map(|chunk| {
                let mut payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();
                payload.insert("document_id".to_string(), chunk.document_id.into());
                payload.insert("chunk_index".to_string(), (chunk.chunk_index as i64).into());
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
                    payload.insert("line_start".to_string(), (line_start as i64).into());
                }
                if let Some(line_end) = chunk.line_end {
                    payload.insert("line_end".to_string(), (line_end as i64).into());
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

    pub async fn search(
        &self,
        query_vector: Vec<f32>,
        limit: u64,
        tags: &[Tag],
        source_types: &[SourceType],
        min_score: Option<f32>,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        let mut filter_conditions = Vec::new();

        for tag in tags {
            filter_conditions.push(Condition::matches("tags", tag.to_payload_string()));
        }

        for st in source_types {
            filter_conditions.push(Condition::matches("source_type", st.to_string()));
        }

        let filter = if filter_conditions.is_empty() {
            None
        } else {
            Some(Filter::must(filter_conditions))
        };

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
                    context_before: None,
                    context_after: None,
                    line_start,
                    line_end,
                }
            })
            .collect();

        Ok(search_results)
    }

    pub async fn delete_by_tags(&self, tags: &[Tag]) -> Result<u64, VectorStoreError> {
        if tags.is_empty() {
            return Ok(0);
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

        Ok(0)
    }

    pub async fn delete_by_document_ids(
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

    pub async fn clear_collection(&self) -> Result<(), VectorStoreError> {
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

    pub async fn delete_by_source_type(&self, source_type: SourceType) -> Result<(), VectorStoreError> {
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

    pub fn collection(&self) -> &str {
        &self.collection
    }
}

#[derive(Debug, Clone)]
pub struct CollectionInfo {
    pub points_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_dim() {
        assert_eq!(EMBEDDING_DIM, 1024);
    }
}
