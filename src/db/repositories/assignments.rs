use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

// ─── Source type constants ──────────────────────────────────────────────────

pub const SOURCE_ADMIN_UPLOAD: &str = "admin_upload";
pub const SOURCE_SYSTEM_TOPIC: &str = "system_topic";
pub const SOURCE_AI_GENERATED: &str = "ai_generated";

/// Default trackable source types used when no config override is given.
pub fn default_trackable_source_types() -> Vec<String> {
    vec![
        SOURCE_SYSTEM_TOPIC.to_string(),
        SOURCE_AI_GENERATED.to_string(),
    ]
}

// ─── Struct ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SystemRecommendationAssignment {
    pub id: i64,
    pub user_id: i64,
    pub recommendation_key: String,
    pub recommendation_item_id: String,
    pub source_type: String,
    pub source_reference: String,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
    pub first_distributed_at: DateTime<Utc>,
    pub last_distributed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── Trait ──────────────────────────────────────────────────────────────────

/// A lightweight representation of a feed item for tracking purposes.
/// Matches the fields needed by `buildAssignmentRow` in the PHP service.
#[derive(Debug, Clone)]
pub struct TrackableFeedItem {
    pub id: String,
    pub source_type: String,
    pub source_reference: Option<String>,
    pub source_payload_topic_id: Option<String>,
    pub source_payload_source_reference: Option<String>,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
}

#[async_trait]
pub trait SystemRecommendationAssignmentsRepo: Send + Sync {
    /// UPSERT a single assignment row.
    /// On conflict `(user_id, recommendation_key)`, updates tracking fields.
    async fn upsert(
        &self,
        user_id: i64,
        recommendation_key: &str,
        recommendation_item_id: &str,
        source_type: &str,
        source_reference: &str,
        subject_id: Option<i64>,
        sub_subject_id: Option<i64>,
        moment: &DateTime<Utc>,
    ) -> anyhow::Result<()>;

    /// Track served recommendations from a list of feed items.
    /// Filters to trackable source types, deduplicates by recommendation_key,
    /// and upserts each row.
    async fn track_served(
        &self,
        user_id: i64,
        items: &[TrackableFeedItem],
        trackable_source_types: &[String],
        moment: &DateTime<Utc>,
    ) -> anyhow::Result<()>;

    /// Find all assignments for a given user.
    async fn find_by_user(&self, user_id: i64) -> anyhow::Result<Vec<SystemRecommendationAssignment>>;

    /// Aggregate assignments by source type / reference for distribution summary.
    async fn find_distribution_summary(
        &self,
        source_types: &[String],
        minimum_distinct_user_count: i64,
    ) -> anyhow::Result<Vec<AssignmentDistributionRow>>;
}

/// Aggregated row for distribution summary.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AssignmentDistributionRow {
    pub source_type: String,
    pub source_reference: String,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
    pub distinct_user_count: i64,
    pub latest_distribution_at: Option<DateTime<Utc>>,
}

// ─── Pg implementation ──────────────────────────────────────────────────────

pub struct PgSystemRecommendationAssignmentsRepo {
    pool: PgPool,
}

impl PgSystemRecommendationAssignmentsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SystemRecommendationAssignmentsRepo for PgSystemRecommendationAssignmentsRepo {
    async fn upsert(
        &self,
        user_id: i64,
        recommendation_key: &str,
        recommendation_item_id: &str,
        source_type: &str,
        source_reference: &str,
        subject_id: Option<i64>,
        sub_subject_id: Option<i64>,
        moment: &DateTime<Utc>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO system_recommendation_assignments
                (user_id, recommendation_key, recommendation_item_id,
                 source_type, source_reference, subject_id, sub_subject_id,
                 first_distributed_at, last_distributed_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (user_id, recommendation_key)
            DO UPDATE SET
                recommendation_item_id = EXCLUDED.recommendation_item_id,
                source_type = EXCLUDED.source_type,
                source_reference = EXCLUDED.source_reference,
                subject_id = EXCLUDED.subject_id,
                sub_subject_id = EXCLUDED.sub_subject_id,
                last_distributed_at = EXCLUDED.last_distributed_at,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(user_id)
        .bind(recommendation_key)
        .bind(recommendation_item_id)
        .bind(source_type)
        .bind(source_reference)
        .bind(subject_id)
        .bind(sub_subject_id)
        .bind(moment)
        .bind(moment)
        .bind(moment)
        .bind(moment)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to upsert system recommendation assignment: {e}"))?;

        Ok(())
    }

    async fn track_served(
        &self,
        user_id: i64,
        items: &[TrackableFeedItem],
        trackable_source_types: &[String],
        moment: &DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let trackable_set: std::collections::HashSet<&str> =
            trackable_source_types.iter().map(|s| s.as_str()).collect();

        // Build rows, filter trackable, deduplicate by recommendation_key
        let mut seen_keys = std::collections::HashSet::new();

        for item in items {
            if !trackable_set.contains(item.source_type.as_str()) {
                continue;
            }

            let source_reference = resolve_source_reference(item);
            let recommendation_key = make_recommendation_key(&item.source_type, &source_reference);

            if !seen_keys.insert(recommendation_key.clone()) {
                continue;
            }

            self.upsert(
                user_id,
                &recommendation_key,
                &item.id,
                &item.source_type,
                &source_reference,
                item.subject_id,
                item.sub_subject_id,
                moment,
            )
            .await?;
        }

        Ok(())
    }

    async fn find_by_user(&self, user_id: i64) -> anyhow::Result<Vec<SystemRecommendationAssignment>> {
        let rows = sqlx::query_as::<_, SystemRecommendationAssignment>(
            "SELECT * FROM system_recommendation_assignments WHERE user_id = $1 ORDER BY last_distributed_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to fetch assignments for user {user_id}: {e}"))?;

        Ok(rows)
    }

    async fn find_distribution_summary(
        &self,
        source_types: &[String],
        minimum_distinct_user_count: i64,
    ) -> anyhow::Result<Vec<AssignmentDistributionRow>> {
        let rows = sqlx::query_as::<_, AssignmentDistributionRow>(
            r#"
            SELECT
                source_type,
                source_reference,
                subject_id,
                sub_subject_id,
                COUNT(DISTINCT user_id)::BIGINT AS distinct_user_count,
                MAX(last_distributed_at)::TIMESTAMPTZ AS latest_distribution_at
            FROM system_recommendation_assignments
            WHERE source_type = ANY($1)
              AND source_reference IS NOT NULL
              AND sub_subject_id IS NOT NULL
            GROUP BY source_type, source_reference, subject_id, sub_subject_id
            HAVING COUNT(DISTINCT user_id) >= $2
            "#,
        )
        .bind(source_types)
        .bind(minimum_distinct_user_count)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to fetch distribution summary: {e}"))?;

        Ok(rows)
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn make_recommendation_key(source_type: &str, source_reference: &str) -> String {
    format!("{source_type}:{source_reference}")
}

fn resolve_source_reference(item: &TrackableFeedItem) -> String {
    // Priority: source_reference field → source_payload.topic_id (for system_topic)
    // → extract from item.id (system_topic_ prefix) → item.id
    if let Some(ref sr) = item.source_reference {
        if !sr.is_empty() {
            return sr.clone();
        }
    }

    if item.source_type == SOURCE_SYSTEM_TOPIC {
        if let Some(ref topic_id) = item.source_payload_topic_id {
            if !topic_id.is_empty() {
                return topic_id.clone();
            }
        }

        if let Some(extracted) = extract_topic_reference_from_item_id(&item.id) {
            return extracted;
        }
    }

    if let Some(ref psr) = item.source_payload_source_reference {
        if !psr.is_empty() {
            return psr.clone();
        }
    }

    item.id.clone()
}

fn extract_topic_reference_from_item_id(item_id: &str) -> Option<String> {
    let prefix = "system_topic_";
    if let Some(topic_id) = item_id.strip_prefix(prefix) {
        if !topic_id.is_empty() {
            return Some(topic_id.to_string());
        }
    }
    None
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_recommendation_key() {
        assert_eq!(
            make_recommendation_key("system_topic", "topic-abc"),
            "system_topic:topic-abc"
        );
    }

    #[test]
    fn test_extract_topic_reference_from_item_id() {
        assert_eq!(
            extract_topic_reference_from_item_id("system_topic_topic-123"),
            Some("topic-123".to_string())
        );
        assert_eq!(
            extract_topic_reference_from_item_id("admin_curated_1"),
            None
        );
        assert_eq!(extract_topic_reference_from_item_id("system_topic_"), None);
    }

    #[test]
    fn test_resolve_source_reference_uses_source_reference_field() {
        let item = TrackableFeedItem {
            id: "system_topic_topic-abc".to_string(),
            source_type: SOURCE_SYSTEM_TOPIC.to_string(),
            source_reference: Some("topic-abc".to_string()),
            source_payload_topic_id: None,
            source_payload_source_reference: None,
            subject_id: None,
            sub_subject_id: None,
        };
        assert_eq!(resolve_source_reference(&item), "topic-abc");
    }

    #[test]
    fn test_resolve_source_reference_falls_back_to_topic_id() {
        let item = TrackableFeedItem {
            id: "system_topic_topic-abc".to_string(),
            source_type: SOURCE_SYSTEM_TOPIC.to_string(),
            source_reference: None,
            source_payload_topic_id: Some("topic-xyz".to_string()),
            source_payload_source_reference: None,
            subject_id: None,
            sub_subject_id: None,
        };
        assert_eq!(resolve_source_reference(&item), "topic-xyz");
    }

    #[test]
    fn test_resolve_source_reference_falls_back_to_item_id_extraction() {
        let item = TrackableFeedItem {
            id: "system_topic_topic-abc".to_string(),
            source_type: SOURCE_SYSTEM_TOPIC.to_string(),
            source_reference: None,
            source_payload_topic_id: None,
            source_payload_source_reference: None,
            subject_id: None,
            sub_subject_id: None,
        };
        assert_eq!(resolve_source_reference(&item), "topic-abc");
    }

    #[test]
    fn test_resolve_source_reference_falls_back_to_item_id() {
        let item = TrackableFeedItem {
            id: "proj-42".to_string(),
            source_type: SOURCE_AI_GENERATED.to_string(),
            source_reference: None,
            source_payload_topic_id: None,
            source_payload_source_reference: None,
            subject_id: None,
            sub_subject_id: None,
        };
        assert_eq!(resolve_source_reference(&item), "proj-42");
    }

    #[test]
    fn test_default_trackable_source_types() {
        let types = default_trackable_source_types();
        assert!(types.contains(&SOURCE_SYSTEM_TOPIC.to_string()));
        assert!(types.contains(&SOURCE_AI_GENERATED.to_string()));
        assert!(!types.contains(&SOURCE_ADMIN_UPLOAD.to_string()));
    }
}
