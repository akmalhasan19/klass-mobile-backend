use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

// ─── Main struct ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MediaGeneration {
    pub id: Uuid,
    pub generated_from_id: Option<Uuid>,
    pub is_regeneration: bool,
    pub teacher_id: i64,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
    pub topic_id: Option<Uuid>,
    pub content_id: Option<Uuid>,
    pub recommended_project_id: Option<i64>,
    pub raw_prompt: String,
    pub request_fingerprint: String,
    pub active_duplicate_key: Option<String>,
    pub preferred_output_type: String,
    pub resolved_output_type: Option<String>,
    pub status: String,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub generator_provider: Option<String>,
    pub generator_model: Option<String>,
    pub interpretation_payload: Option<serde_json::Value>,
    pub interpretation_audit_payload: Option<serde_json::Value>,
    pub generation_spec_payload: Option<serde_json::Value>,
    pub decision_payload: Option<serde_json::Value>,
    pub orchestration_audit_payload: Option<serde_json::Value>,
    pub delivery_payload: Option<serde_json::Value>,
    pub generator_service_response: Option<serde_json::Value>,
    pub storage_path: Option<String>,
    pub file_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub mime_type: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    // -- Async media generation job tracking (Task 1.1 / Task 1.2) --
    pub generation_job_id: Option<Uuid>,
    pub generation_status: Option<String>,
    pub s3_object_key: Option<String>,
    pub presigned_download_url: Option<String>,
    pub presigned_url_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub generation_error_code: Option<String>,
    pub generation_error_message: Option<String>,
    // -- Prompt clarification fields (Phase 2) --
    pub clarification_state: Option<serde_json::Value>,
    pub clarified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub clarification_skipped: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ─── Summary structs for eager-loaded relations ───────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SubjectSummary {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SubSubjectSummary {
    pub id: i64,
    pub subject_id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TopicSummary {
    pub id: Uuid,
    pub title: String,
    pub sub_subject_id: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub is_published: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ContentSummary {
    pub id: Uuid,
    pub topic_id: Uuid,
    #[sqlx(rename = "type")]
    pub content_type: String,
    pub title: Option<String>,
    pub media_url: Option<String>,
    pub is_published: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecommendedProjectSummary {
    pub id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub project_file_url: Option<String>,
    pub source_type: String,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct SubSubjectWithSubject {
    pub sub_subject: SubSubjectSummary,
    pub subject: SubjectSummary,
}

// ─── Composite struct ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MediaGenerationWithRelations {
    pub generation: MediaGeneration,
    pub subject: Option<SubjectSummary>,
    pub sub_subject: Option<SubSubjectWithSubject>,
    pub topic: Option<TopicSummary>,
    pub content: Option<ContentSummary>,
    pub recommended_project: Option<RecommendedProjectSummary>,
}

// ─── Chain result ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MediaGenerationChain {
    /// Ancestors from the given generation up to the root (oldest-first).
    pub ancestors: Vec<MediaGeneration>,
    /// Direct children of the given generation (oldest-first).
    pub children: Vec<MediaGeneration>,
}

// ─── Input payloads ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CreateMediaGenerationPayload {
    pub teacher_id: i64,
    pub raw_prompt: String,
    pub request_fingerprint: String,
    pub generated_from_id: Option<Uuid>,
    pub is_regeneration: bool,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
    pub preferred_output_type: String,
    pub active_duplicate_key: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdatePayloadsPayload {
    pub interpretation_payload: Option<serde_json::Value>,
    pub interpretation_audit_payload: Option<serde_json::Value>,
    pub generation_spec_payload: Option<serde_json::Value>,
    pub decision_payload: Option<serde_json::Value>,
    pub orchestration_audit_payload: Option<serde_json::Value>,
    pub delivery_payload: Option<serde_json::Value>,
    pub generator_service_response: Option<serde_json::Value>,
    pub resolved_output_type: Option<String>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub generator_provider: Option<String>,
    pub generator_model: Option<String>,
    pub storage_path: Option<String>,
    pub file_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub mime_type: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

// ─── Async media generation job payloads (Task 1.2) ───────────────────────────

#[derive(Debug, Clone)]
pub struct UpdateGenerationJobStatusPayload {
    pub generation_job_id: Option<Uuid>,
    pub generation_status: String,
}

#[derive(Debug, Clone)]
pub struct UpdateS3MetadataPayload {
    pub s3_object_key: String,
    pub presigned_download_url: String,
    pub presigned_url_expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct UpdateGenerationErrorPayload {
    pub generation_error_code: String,
    pub generation_error_message: String,
}

// ─── Clarification payloads (Phase 2) ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UpdateClarificationStatePayload {
    pub clarification_state: Option<serde_json::Value>,
    pub clarified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub clarification_skipped: bool,
}

// ─── Admin listing types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct AdminMediaGenerationFilters {
    pub status: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MediaGenerationAdminRow {
    pub id: Uuid,
    pub generated_from_id: Option<Uuid>,
    pub is_regeneration: bool,
    pub teacher_id: i64,
    pub raw_prompt: String,
    pub preferred_output_type: String,
    pub resolved_output_type: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub teacher_name: String,
    pub teacher_email: String,
}

// ─── Column list ──────────────────────────────────────────────────────────────

const MG_SELECT_COLS: &str = r#"
    id, generated_from_id, is_regeneration, teacher_id,
    subject_id, sub_subject_id, topic_id, content_id, recommended_project_id,
    raw_prompt, request_fingerprint, active_duplicate_key,
    preferred_output_type, resolved_output_type, status,
    llm_provider, llm_model, generator_provider, generator_model,
    interpretation_payload, interpretation_audit_payload,
    generation_spec_payload, decision_payload,
    orchestration_audit_payload, delivery_payload,
    generator_service_response, storage_path, file_url, thumbnail_url,
    mime_type, error_code, error_message,
    generation_job_id, generation_status, s3_object_key,
    presigned_download_url, presigned_url_expires_at,
    generation_error_code, generation_error_message,
    clarification_state, clarified_at, clarification_skipped,
    created_at, updated_at
"#;

// ─── Trait ────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait MediaGenerationsRepo: Send + Sync {
    /// Fetch the most recent media generations for a teacher, with eager-loaded
    /// subject, sub_subject (with subject), topic, content, and recommended_project.
    async fn find_recent_for_teacher(
        &self,
        teacher_id: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<MediaGenerationWithRelations>>;

    /// Fetch a single media generation by id, scoped to the teacher.
    async fn find_by_id_for_teacher(
        &self,
        id: Uuid,
        teacher_id: i64,
    ) -> anyhow::Result<Option<MediaGenerationWithRelations>>;

    /// Walk the generation chain: ancestors from the given id up to root (max 50),
    /// plus direct children (oldest-first).
    async fn find_chain(&self, id: Uuid) -> anyhow::Result<MediaGenerationChain>;

    /// Insert a new media generation. Returns the created row.
    async fn insert(&self, payload: &CreateMediaGenerationPayload) -> anyhow::Result<MediaGeneration>;

    /// Update only the status field. Returns the updated row.
    async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<MediaGeneration>;

    /// Update multiple payload/result fields at once. Returns the updated row.
    async fn update_payloads(&self, id: Uuid, payload: &UpdatePayloadsPayload) -> anyhow::Result<MediaGeneration>;

    // ─── Async media generation job tracking (Task 1.2) ───────────────────────

    /// Update `generation_job_id` and `generation_status` for a generation.
    async fn update_generation_job_status(
        &self,
        id: Uuid,
        payload: &UpdateGenerationJobStatusPayload,
    ) -> anyhow::Result<MediaGeneration>;

    /// Find a generation by its `generation_job_id`.
    async fn find_by_job_id(&self, job_id: Uuid) -> anyhow::Result<Option<MediaGeneration>>;

    /// Update S3 metadata fields: `s3_object_key`, `presigned_download_url`, `presigned_url_expires_at`.
    async fn update_s3_metadata(
        &self,
        id: Uuid,
        payload: &UpdateS3MetadataPayload,
    ) -> anyhow::Result<MediaGeneration>;

    /// Update generation error fields: `generation_error_code`, `generation_error_message`.
    async fn update_generation_error(
        &self,
        id: Uuid,
        payload: &UpdateGenerationErrorPayload,
    ) -> anyhow::Result<MediaGeneration>;

    // ─── Prompt clarification (Phase 2) ───────────────────────────────────────

    /// Update clarification state for a generation.
    /// Sets `clarification_state`, `clarified_at`, and `clarification_skipped`.
    async fn update_clarification_state(
        &self,
        id: Uuid,
        payload: &UpdateClarificationStatePayload,
    ) -> anyhow::Result<MediaGeneration>;

    /// Reset a generation for reprocessing with a new prompt.
    ///
    /// Updates `raw_prompt`, clears all classification payloads
    /// (interpretation, decision, spec, delivery), and resets `status` to `queued`.
    /// Used by the clarification flow to re-trigger the full LLM pipeline
    /// (interpret → decide → draft → generate) on the enriched prompt.
    async fn reset_for_reprocessing(
        &self,
        id: Uuid,
        new_raw_prompt: &str,
    ) -> anyhow::Result<MediaGeneration>;
}

// ─── Pg implementation ────────────────────────────────────────────────────────

pub struct PgMediaGenerationsRepo {
    pool: PgPool,
}

impl PgMediaGenerationsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PgMediaGenerationsRepo {
    /// Fetch a single row by id (unscoped). Used internally.
    pub async fn find_raw(&self, id: Uuid) -> anyhow::Result<MediaGeneration> {
        let sql = format!(
            r#"SELECT {MG_SELECT_COLS} FROM media_generations WHERE id = $1"#,
        );
        sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch media generation: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("media generation not found"))
    }
}

impl PgMediaGenerationsRepo {
    /// Fetch paginated media generations for the admin listing.
    ///
    /// Supports filtering by `status` (exact match) and `search`
    /// (ILIKE against `id::text`, `raw_prompt`, teacher `name`, teacher `email`).
    /// Includes teacher name/email via JOIN with `users`.
    pub async fn find_all_admin(
        &self,
        filters: &AdminMediaGenerationFilters,
        pagination: &crate::db::pagination::PaginationQuery,
    ) -> anyhow::Result<(Vec<MediaGenerationAdminRow>, i64)> {
        let search_pattern = filters
            .search
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", s));

        let count_sql = r#"
            SELECT COUNT(*)
            FROM media_generations mg
            LEFT JOIN users u ON mg.teacher_id = u.id
            WHERE ($1::text IS NULL OR mg.status = $1)
              AND ($2::text IS NULL OR
                   mg.id::text ILIKE $2 OR
                   mg.raw_prompt ILIKE $2 OR
                   u.name ILIKE $2 OR
                   u.email ILIKE $2)
        "#;

        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(filters.status.as_deref())
            .bind(search_pattern.as_deref())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to count media generations: {e}"))?;

        let data_sql = r#"
            SELECT mg.id, mg.generated_from_id, mg.is_regeneration, mg.teacher_id,
                   mg.raw_prompt, mg.preferred_output_type, mg.resolved_output_type,
                   mg.status, mg.error_code, mg.error_message,
                   mg.created_at, mg.updated_at,
                   u.name AS teacher_name, u.email AS teacher_email
            FROM media_generations mg
            LEFT JOIN users u ON mg.teacher_id = u.id
            WHERE ($1::text IS NULL OR mg.status = $1)
              AND ($2::text IS NULL OR
                   mg.id::text ILIKE $2 OR
                   mg.raw_prompt ILIKE $2 OR
                   u.name ILIKE $2 OR
                   u.email ILIKE $2)
            ORDER BY mg.created_at DESC
            LIMIT $3 OFFSET $4
        "#;

        let rows: Vec<MediaGenerationAdminRow> = sqlx::query_as(&data_sql)
            .bind(filters.status.as_deref())
            .bind(search_pattern.as_deref())
            .bind(pagination.limit())
            .bind(pagination.offset())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch media generations: {e}"))?;

        Ok((rows, total))
    }
}

#[async_trait]
impl MediaGenerationsRepo for PgMediaGenerationsRepo {
    async fn find_recent_for_teacher(
        &self,
        teacher_id: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<MediaGenerationWithRelations>> {
        let sql = format!(
            r#"SELECT {MG_SELECT_COLS}
               FROM media_generations
               WHERE teacher_id = $1
               ORDER BY created_at DESC
               LIMIT $2"#,
        );

        let generations: Vec<MediaGeneration> = sqlx::query_as(&sql)
            .bind(teacher_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch recent media generations: {e}"))?;

        attach_relations(&self.pool, generations).await
    }

    async fn find_by_id_for_teacher(
        &self,
        id: Uuid,
        teacher_id: i64,
    ) -> anyhow::Result<Option<MediaGenerationWithRelations>> {
        let sql = format!(
            r#"SELECT {MG_SELECT_COLS}
               FROM media_generations
               WHERE id = $1 AND teacher_id = $2"#,
        );

        let generation = sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(id)
            .bind(teacher_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch media generation {id}: {e}"))?;

        let generation = match generation {
            Some(g) => g,
            None => return Ok(None),
        };

        let result = attach_relations(&self.pool, vec![generation]).await?;
        Ok(result.into_iter().next())
    }

    async fn find_chain(&self, id: Uuid) -> anyhow::Result<MediaGenerationChain> {
        // Walk ancestors: start from the given id, follow generated_from_id up to root
        let mut ancestors: Vec<MediaGeneration> = Vec::new();
        let mut current_id = Some(id);

        for _ in 0..50 {
            let sql = format!(
                r#"SELECT {MG_SELECT_COLS} FROM media_generations WHERE id = $1"#,
            );

            let gen = sqlx::query_as::<_, MediaGeneration>(&sql)
                .bind(current_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("failed to fetch ancestor in chain: {e}"))?;

            match gen {
                Some(g) => {
                    current_id = g.generated_from_id;
                    ancestors.push(g);

                    if current_id.is_none() {
                        break;
                    }
                }
                None => break,
            }
        }

        // Ancestors are collected from closest to root; reverse so oldest-first
        ancestors.reverse();

        // Fetch direct children (oldest-first)
        let children_sql = format!(
            r#"SELECT {MG_SELECT_COLS}
               FROM media_generations
               WHERE generated_from_id = $1
               ORDER BY created_at ASC"#,
        );

        let children: Vec<MediaGeneration> = sqlx::query_as(&children_sql)
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch children in chain: {e}"))?;

        Ok(MediaGenerationChain {
            ancestors,
            children,
        })
    }

    async fn insert(&self, payload: &CreateMediaGenerationPayload) -> anyhow::Result<MediaGeneration> {
        let sql = format!(
            r#"
            INSERT INTO media_generations
                (id, teacher_id, raw_prompt, request_fingerprint,
                 generated_from_id, is_regeneration,
                 subject_id, sub_subject_id,
                 preferred_output_type, active_duplicate_key)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING {MG_SELECT_COLS}
            "#,
        );

        let generation = sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(Uuid::new_v4())
            .bind(payload.teacher_id)
            .bind(&payload.raw_prompt)
            .bind(&payload.request_fingerprint)
            .bind(payload.generated_from_id)
            .bind(payload.is_regeneration)
            .bind(payload.subject_id)
            .bind(payload.sub_subject_id)
            .bind(&payload.preferred_output_type)
            .bind(&payload.active_duplicate_key)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to insert media generation: {e}"))?;

        Ok(generation)
    }

    async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<MediaGeneration> {
        let sql = format!(
            r#"
            UPDATE media_generations
            SET status = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {MG_SELECT_COLS}
            "#,
        );

        let generation = sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(id)
            .bind(status)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to update media generation status: {e}"))?;

        generation.ok_or_else(|| anyhow::anyhow!("media generation not found"))
    }

    async fn update_payloads(&self, id: Uuid, payload: &UpdatePayloadsPayload) -> anyhow::Result<MediaGeneration> {
        let set_parts = build_payload_set(payload);
        if set_parts.is_empty() {
            return self.find_raw(id).await;
        }

        let set_str = set_parts.join(", ");
        let id_param = set_parts.len() + 1;

        let sql = format!(
            r#"UPDATE media_generations
               SET {set_str}, updated_at = NOW()
               WHERE id = ${id_param}
               RETURNING {MG_SELECT_COLS}"#,
        );

        let mut query = sqlx::query_as::<_, MediaGeneration>(&sql);

        if let Some(ref val) = payload.interpretation_payload {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.interpretation_audit_payload {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.generation_spec_payload {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.decision_payload {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.orchestration_audit_payload {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.delivery_payload {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.generator_service_response {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.resolved_output_type {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.llm_provider {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.llm_model {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.generator_provider {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.generator_model {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.storage_path {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.file_url {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.thumbnail_url {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.mime_type {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.error_code {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.error_message {
            query = query.bind(val);
        }

        let generation = query
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to update media generation payloads: {e}"))?;

        generation.ok_or_else(|| anyhow::anyhow!("media generation not found"))
    }

    // ─── Async media generation job tracking (Task 1.2) ───────────────────────

    async fn update_generation_job_status(
        &self,
        id: Uuid,
        payload: &UpdateGenerationJobStatusPayload,
    ) -> anyhow::Result<MediaGeneration> {
        let sql = format!(
            r#"
            UPDATE media_generations
            SET generation_job_id = $2,
                generation_status = $3,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {MG_SELECT_COLS}
            "#,
        );

        let generation = sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(id)
            .bind(payload.generation_job_id)
            .bind(&payload.generation_status)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                anyhow::anyhow!("failed to update generation job status for {id}: {e}")
            })?;

        generation.ok_or_else(|| anyhow::anyhow!("media generation {id} not found"))
    }

    async fn find_by_job_id(&self, job_id: Uuid) -> anyhow::Result<Option<MediaGeneration>> {
        let sql = format!(
            r#"SELECT {MG_SELECT_COLS}
               FROM media_generations
               WHERE generation_job_id = $1"#,
        );

        let generation = sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to find media generation by job_id {job_id}: {e}"))?;

        Ok(generation)
    }

    async fn update_s3_metadata(
        &self,
        id: Uuid,
        payload: &UpdateS3MetadataPayload,
    ) -> anyhow::Result<MediaGeneration> {
        let sql = format!(
            r#"
            UPDATE media_generations
            SET s3_object_key = $2,
                presigned_download_url = $3,
                presigned_url_expires_at = $4,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {MG_SELECT_COLS}
            "#,
        );

        let generation = sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(id)
            .bind(&payload.s3_object_key)
            .bind(&payload.presigned_download_url)
            .bind(payload.presigned_url_expires_at)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to update S3 metadata for {id}: {e}"))?;

        generation.ok_or_else(|| anyhow::anyhow!("media generation {id} not found"))
    }

    async fn update_generation_error(
        &self,
        id: Uuid,
        payload: &UpdateGenerationErrorPayload,
    ) -> anyhow::Result<MediaGeneration> {
        let sql = format!(
            r#"
            UPDATE media_generations
            SET generation_error_code = $2,
                generation_error_message = $3,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {MG_SELECT_COLS}
            "#,
        );

        let generation = sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(id)
            .bind(&payload.generation_error_code)
            .bind(&payload.generation_error_message)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to update generation error for {id}: {e}"))?;

        generation.ok_or_else(|| anyhow::anyhow!("media generation {id} not found"))
    }

    // ─── Prompt clarification (Phase 2) ───────────────────────────────────────

    async fn update_clarification_state(
        &self,
        id: Uuid,
        payload: &UpdateClarificationStatePayload,
    ) -> anyhow::Result<MediaGeneration> {
        let sql = format!(
            r#"
            UPDATE media_generations
            SET clarification_state = $2,
                clarified_at = $3,
                clarification_skipped = $4,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {MG_SELECT_COLS}
            "#,
        );

        let generation = sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(id)
            .bind(&payload.clarification_state)
            .bind(payload.clarified_at)
            .bind(payload.clarification_skipped)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to update clarification state for {id}: {e}"))?;

        generation.ok_or_else(|| anyhow::anyhow!("media generation {id} not found"))
    }

    async fn reset_for_reprocessing(
        &self,
        id: Uuid,
        new_raw_prompt: &str,
    ) -> anyhow::Result<MediaGeneration> {
        let sql = format!(
            r#"
            UPDATE media_generations
            SET raw_prompt = $2,
                status = 'queued',
                interpretation_payload = NULL,
                interpretation_audit_payload = NULL,
                generation_spec_payload = NULL,
                decision_payload = NULL,
                orchestration_audit_payload = NULL,
                delivery_payload = NULL,
                generator_service_response = NULL,
                resolved_output_type = NULL,
                llm_provider = NULL,
                llm_model = NULL,
                generator_provider = NULL,
                generator_model = NULL,
                error_code = NULL,
                error_message = NULL,
                generation_status = NULL,
                generation_error_code = NULL,
                generation_error_message = NULL,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {MG_SELECT_COLS}
            "#,
        );

        let generation = sqlx::query_as::<_, MediaGeneration>(&sql)
            .bind(id)
            .bind(new_raw_prompt)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to reset generation for reprocessing {id}: {e}"))?;

        generation.ok_or_else(|| anyhow::anyhow!("media generation {id} not found"))
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Eager-load relations for a batch of media generations.
async fn attach_relations(
    pool: &PgPool,
    generations: Vec<MediaGeneration>,
) -> anyhow::Result<Vec<MediaGenerationWithRelations>> {
    if generations.is_empty() {
        return Ok(Vec::new());
    }

    // Collect relation IDs
    let subject_ids: Vec<i64> = generations
        .iter()
        .filter_map(|g| g.subject_id)
        .collect();
    let sub_subject_ids: Vec<i64> = generations
        .iter()
        .filter_map(|g| g.sub_subject_id)
        .collect();
    let topic_ids: Vec<Uuid> = generations
        .iter()
        .filter_map(|g| g.topic_id)
        .collect();
    let content_ids: Vec<Uuid> = generations
        .iter()
        .filter_map(|g| g.content_id)
        .collect();
    let recommended_project_ids: Vec<i64> = generations
        .iter()
        .filter_map(|g| g.recommended_project_id)
        .collect();

    // Fetch all relations in parallel
    let (subjects, sub_subjects, topics, contents, recommended_projects) = tokio::join!(
        fetch_subjects(pool, &subject_ids),
        fetch_sub_subjects(pool, &sub_subject_ids),
        fetch_topics(pool, &topic_ids),
        fetch_contents(pool, &content_ids),
        fetch_recommended_projects(pool, &recommended_project_ids),
    );

    let subjects = subjects?;
    let sub_subjects = sub_subjects?;
    let topics = topics?;
    let contents = contents?;
    let recommended_projects = recommended_projects?;

    // Build lookup maps
    let subject_map: std::collections::HashMap<i64, SubjectSummary> =
        subjects.into_iter().map(|s| (s.id, s)).collect();
    let sub_subject_map: std::collections::HashMap<i64, SubSubjectSummary> =
        sub_subjects.into_iter().map(|s| (s.id, s)).collect();
    let topic_map: std::collections::HashMap<Uuid, TopicSummary> =
        topics.into_iter().map(|t| (t.id, t)).collect();
    let content_map: std::collections::HashMap<Uuid, ContentSummary> =
        contents.into_iter().map(|c| (c.id, c)).collect();
    let recommended_project_map: std::collections::HashMap<i64, RecommendedProjectSummary> =
        recommended_projects.into_iter().map(|r| (r.id, r)).collect();

    // Compose
    let result = generations
        .into_iter()
        .map(|g| {
            let subj = g.subject_id.and_then(|id| subject_map.get(&id).cloned());
            let sub_subj = g.sub_subject_id.and_then(|id| {
                sub_subject_map.get(&id).cloned().map(|ss| {
                    let s_subj = subject_map.get(&ss.subject_id).cloned();
                    SubSubjectWithSubject {
                        sub_subject: ss,
                        subject: s_subj.unwrap_or_else(|| SubjectSummary {
                            id: 0,
                            name: String::new(),
                            slug: String::new(),
                        }),
                    }
                })
            });

            MediaGenerationWithRelations {
                subject: subj,
                sub_subject: sub_subj,
                topic: g.topic_id.and_then(|id| topic_map.get(&id).cloned()),
                content: g.content_id.and_then(|id| content_map.get(&id).cloned()),
                recommended_project: g
                    .recommended_project_id
                    .and_then(|id| recommended_project_map.get(&id).cloned()),
                generation: g,
            }
        })
        .collect();

    Ok(result)
}

async fn fetch_subjects(pool: &PgPool, ids: &[i64]) -> anyhow::Result<Vec<SubjectSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let subjects = sqlx::query_as::<_, SubjectSummary>(
        r#"SELECT id, name, slug FROM subjects WHERE id = ANY($1)"#,
    )
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|e| anyhow::anyhow!("failed to fetch subjects: {e}"))?;
    Ok(subjects)
}

async fn fetch_sub_subjects(pool: &PgPool, ids: &[i64]) -> anyhow::Result<Vec<SubSubjectSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let sub_subjects = sqlx::query_as::<_, SubSubjectSummary>(
        r#"SELECT id, subject_id, name, slug FROM sub_subjects WHERE id = ANY($1)"#,
    )
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|e| anyhow::anyhow!("failed to fetch sub_subjects: {e}"))?;
    Ok(sub_subjects)
}

async fn fetch_topics(pool: &PgPool, ids: &[Uuid]) -> anyhow::Result<Vec<TopicSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let topics = sqlx::query_as::<_, TopicSummary>(
        r#"SELECT id, title, sub_subject_id, thumbnail_url, is_published FROM topics WHERE id = ANY($1)"#,
    )
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|e| anyhow::anyhow!("failed to fetch topics: {e}"))?;
    Ok(topics)
}

async fn fetch_contents(pool: &PgPool, ids: &[Uuid]) -> anyhow::Result<Vec<ContentSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let contents = sqlx::query_as::<_, ContentSummary>(
        r#"SELECT id, topic_id, type, title, media_url, is_published FROM contents WHERE id = ANY($1)"#,
    )
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|e| anyhow::anyhow!("failed to fetch contents: {e}"))?;
    Ok(contents)
}

async fn fetch_recommended_projects(
    pool: &PgPool,
    ids: &[i64],
) -> anyhow::Result<Vec<RecommendedProjectSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let projects = sqlx::query_as::<_, RecommendedProjectSummary>(
        r#"SELECT id, title, thumbnail_url, project_file_url, source_type, is_active
           FROM recommended_projects WHERE id = ANY($1)"#,
    )
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|e| anyhow::anyhow!("failed to fetch recommended projects: {e}"))?;
    Ok(projects)
}

/// Build the dynamic SET clause parts for `update_payloads`.
#[allow(unused_assignments)]
fn build_payload_set(payload: &UpdatePayloadsPayload) -> Vec<String> {
    let mut parts = Vec::new();
    let mut idx = 1u32;

    macro_rules! add_col {
        ($field:ident, $col:literal) => {
            if payload.$field.is_some() {
                parts.push(format!("{} = ${}", $col, idx));
                idx += 1;
            }
        };
    }

    add_col!(interpretation_payload, "interpretation_payload");
    add_col!(interpretation_audit_payload, "interpretation_audit_payload");
    add_col!(generation_spec_payload, "generation_spec_payload");
    add_col!(decision_payload, "decision_payload");
    add_col!(orchestration_audit_payload, "orchestration_audit_payload");
    add_col!(delivery_payload, "delivery_payload");
    add_col!(generator_service_response, "generator_service_response");
    add_col!(resolved_output_type, "resolved_output_type");
    add_col!(llm_provider, "llm_provider");
    add_col!(llm_model, "llm_model");
    add_col!(generator_provider, "generator_provider");
    add_col!(generator_model, "generator_model");
    add_col!(storage_path, "storage_path");
    add_col!(file_url, "file_url");
    add_col!(thumbnail_url, "thumbnail_url");
    add_col!(mime_type, "mime_type");
    add_col!(error_code, "error_code");
    add_col!(error_message, "error_message");

    parts
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_generation_struct_fields() {
        let gen = MediaGeneration {
            id: Uuid::new_v4(),
            generated_from_id: None,
            is_regeneration: false,
            teacher_id: 1,
            subject_id: None,
            sub_subject_id: None,
            topic_id: None,
            content_id: None,
            recommended_project_id: None,
            raw_prompt: "Buatkan materi pecahan".to_string(),
            request_fingerprint: "abc123".to_string(),
            active_duplicate_key: None,
            preferred_output_type: "auto".to_string(),
            resolved_output_type: None,
            status: "queued".to_string(),
            llm_provider: None,
            llm_model: None,
            generator_provider: None,
            generator_model: None,
            interpretation_payload: None,
            interpretation_audit_payload: None,
            generation_spec_payload: None,
            decision_payload: None,
            orchestration_audit_payload: None,
            delivery_payload: None,
            generator_service_response: None,
            storage_path: None,
            file_url: None,
            thumbnail_url: None,
            mime_type: None,
            error_code: None,
            error_message: None,
            generation_job_id: None,
            generation_status: None,
            s3_object_key: None,
            presigned_download_url: None,
            presigned_url_expires_at: None,
            generation_error_code: None,
            generation_error_message: None,
            clarification_state: None,
            clarified_at: None,
            clarification_skipped: false,
            created_at: None,
            updated_at: None,
        };

        assert_eq!(gen.raw_prompt, "Buatkan materi pecahan");
        assert_eq!(gen.status, "queued");
        assert!(!gen.is_regeneration);
        assert!(!gen.clarification_skipped);
    }

    #[test]
    fn test_create_payload_defaults() {
        let payload = CreateMediaGenerationPayload {
            teacher_id: 42,
            raw_prompt: "Test prompt".to_string(),
            request_fingerprint: "fingerprint_1".to_string(),
            generated_from_id: None,
            is_regeneration: false,
            subject_id: None,
            sub_subject_id: None,
            preferred_output_type: "auto".to_string(),
            active_duplicate_key: None,
        };

        assert_eq!(payload.teacher_id, 42);
        assert_eq!(payload.preferred_output_type, "auto");
        assert!(!payload.is_regeneration);
    }

    #[test]
    fn test_update_payloads_payload_default() {
        let payload = UpdatePayloadsPayload::default();
        assert!(payload.resolved_output_type.is_none());
        assert!(payload.llm_provider.is_none());
    }

    #[test]
    fn test_update_payloads_with_values() {
        let payload = UpdatePayloadsPayload {
            interpretation_payload: Some(serde_json::json!({"key": "value"})),
            resolved_output_type: Some("pdf".to_string()),
            llm_provider: Some("openrouter".to_string()),
            ..Default::default()
        };

        assert!(payload.interpretation_payload.is_some());
        assert_eq!(payload.resolved_output_type.as_deref(), Some("pdf"));
        assert_eq!(payload.llm_provider.as_deref(), Some("openrouter"));
        assert!(payload.delivery_payload.is_none());
    }

    #[test]
    fn test_subject_summary_struct() {
        let subject = SubjectSummary {
            id: 1,
            name: "Matematika".to_string(),
            slug: "matematika".to_string(),
        };
        assert_eq!(subject.name, "Matematika");
    }

    #[test]
    fn test_sub_subject_with_subject_struct() {
        let sub_subject = SubSubjectSummary {
            id: 10,
            subject_id: 1,
            name: "Pecahan".to_string(),
            slug: "pecahan".to_string(),
        };
        let subject = SubjectSummary {
            id: 1,
            name: "Matematika".to_string(),
            slug: "matematika".to_string(),
        };
        let sws = SubSubjectWithSubject {
            sub_subject,
            subject,
        };
        assert_eq!(sws.sub_subject.name, "Pecahan");
        assert_eq!(sws.subject.name, "Matematika");
    }

    #[test]
    fn test_topic_summary_struct() {
        let topic = TopicSummary {
            id: Uuid::new_v4(),
            title: "Materi Pecahan".to_string(),
            sub_subject_id: Some(10),
            thumbnail_url: None,
            is_published: true,
        };
        assert_eq!(topic.title, "Materi Pecahan");
    }

    #[test]
    fn test_content_summary_struct() {
        let content = ContentSummary {
            id: Uuid::new_v4(),
            topic_id: Uuid::new_v4(),
            content_type: "module".to_string(),
            title: Some("Pengenalan Pecahan".to_string()),
            media_url: None,
            is_published: true,
        };
        assert_eq!(
            content.title.as_deref(),
            Some("Pengenalan Pecahan")
        );
    }

    #[test]
    fn test_recommended_project_summary_struct() {
        let project = RecommendedProjectSummary {
            id: 1,
            title: "Project Matematika".to_string(),
            thumbnail_url: None,
            project_file_url: None,
            source_type: "ai_generated".to_string(),
            is_active: true,
        };
        assert_eq!(project.source_type, "ai_generated");
    }

    #[test]
    fn test_media_generation_with_relations_struct() {
        let gen = MediaGeneration {
            id: Uuid::new_v4(),
            generated_from_id: None,
            is_regeneration: false,
            teacher_id: 1,
            subject_id: None,
            sub_subject_id: None,
            topic_id: None,
            content_id: None,
            recommended_project_id: None,
            raw_prompt: "Test".to_string(),
            request_fingerprint: "fp".to_string(),
            active_duplicate_key: None,
            preferred_output_type: "auto".to_string(),
            resolved_output_type: None,
            status: "queued".to_string(),
            llm_provider: None,
            llm_model: None,
            generator_provider: None,
            generator_model: None,
            interpretation_payload: None,
            interpretation_audit_payload: None,
            generation_spec_payload: None,
            decision_payload: None,
            orchestration_audit_payload: None,
            delivery_payload: None,
            generator_service_response: None,
            storage_path: None,
            file_url: None,
            thumbnail_url: None,
            mime_type: None,
            error_code: None,
            error_message: None,
            generation_job_id: None,
            generation_status: None,
            s3_object_key: None,
            presigned_download_url: None,
            presigned_url_expires_at: None,
            generation_error_code: None,
            generation_error_message: None,
            clarification_state: None,
            clarified_at: None,
            clarification_skipped: false,
            created_at: None,
            updated_at: None,
        };

        let subject = SubjectSummary {
            id: 1,
            name: "Matematika".to_string(),
            slug: "matematika".to_string(),
        };

        let mwr = MediaGenerationWithRelations {
            subject: Some(subject),
            sub_subject: None,
            topic: None,
            content: None,
            recommended_project: None,
            generation: gen,
        };

        assert!(mwr.subject.is_some());
        assert_eq!(mwr.subject.as_ref().unwrap().name, "Matematika");
    }

    #[test]
    fn test_media_generation_chain_struct() {
        let gen = MediaGeneration {
            id: Uuid::new_v4(),
            generated_from_id: None,
            is_regeneration: false,
            teacher_id: 1,
            subject_id: None,
            sub_subject_id: None,
            topic_id: None,
            content_id: None,
            recommended_project_id: None,
            raw_prompt: "Root".to_string(),
            request_fingerprint: "fp".to_string(),
            active_duplicate_key: None,
            preferred_output_type: "auto".to_string(),
            resolved_output_type: None,
            status: "completed".to_string(),
            llm_provider: None,
            llm_model: None,
            generator_provider: None,
            generator_model: None,
            interpretation_payload: None,
            interpretation_audit_payload: None,
            generation_spec_payload: None,
            decision_payload: None,
            orchestration_audit_payload: None,
            delivery_payload: None,
            generator_service_response: None,
            storage_path: None,
            file_url: None,
            thumbnail_url: None,
            mime_type: None,
            error_code: None,
            error_message: None,
            generation_job_id: None,
            generation_status: None,
            s3_object_key: None,
            presigned_download_url: None,
            presigned_url_expires_at: None,
            generation_error_code: None,
            generation_error_message: None,
            clarification_state: None,
            clarified_at: None,
            clarification_skipped: false,
            created_at: None,
            updated_at: None,
        };

        let chain = MediaGenerationChain {
            ancestors: vec![gen.clone()],
            children: vec![],
        };

        assert_eq!(chain.ancestors.len(), 1);
        assert_eq!(chain.ancestors[0].raw_prompt, "Root");
        assert!(chain.children.is_empty());
    }

    #[test]
    fn test_build_payload_set_empty() {
        let payload = UpdatePayloadsPayload::default();
        let parts = build_payload_set(&payload);
        assert!(parts.is_empty());
    }

    #[test]
    fn test_build_payload_set_some_fields() {
        let payload = UpdatePayloadsPayload {
            resolved_output_type: Some("pdf".to_string()),
            llm_provider: Some("openrouter".to_string()),
            interpretation_payload: Some(serde_json::json!({"key": "val"})),
            ..Default::default()
        };
        let parts = build_payload_set(&payload);
        assert_eq!(parts.len(), 3);
        assert!(parts.contains(&"interpretation_payload = $1".to_string()));
    }

    #[test]
    fn test_update_clarification_state_payload() {
        let payload = UpdateClarificationStatePayload {
            clarification_state: Some(serde_json::json!({
                "answers": {"target_audience": "SD_Kelas_5"},
                "suggested_prompt": "Buatkan materi pecahan untuk SD Kelas 5",
            })),
            clarified_at: Some(chrono::Utc::now()),
            clarification_skipped: false,
        };

        assert!(payload.clarification_state.is_some());
        assert!(payload.clarified_at.is_some());
        assert!(!payload.clarification_skipped);
    }

    #[test]
    fn test_update_clarification_state_payload_skipped() {
        let payload = UpdateClarificationStatePayload {
            clarification_state: Some(serde_json::json!({
                "answers": {},
                "skipped": true,
            })),
            clarified_at: Some(chrono::Utc::now()),
            clarification_skipped: true,
        };

        assert!(payload.clarification_skipped);
    }

    #[test]
    fn test_media_generation_clarification_fields() {
        let gen = MediaGeneration {
            id: Uuid::new_v4(),
            generated_from_id: None,
            is_regeneration: false,
            teacher_id: 1,
            subject_id: None,
            sub_subject_id: None,
            topic_id: None,
            content_id: None,
            recommended_project_id: None,
            raw_prompt: "Test".to_string(),
            request_fingerprint: "fp".to_string(),
            active_duplicate_key: None,
            preferred_output_type: "auto".to_string(),
            resolved_output_type: None,
            status: "queued".to_string(),
            llm_provider: None,
            llm_model: None,
            generator_provider: None,
            generator_model: None,
            interpretation_payload: None,
            interpretation_audit_payload: None,
            generation_spec_payload: None,
            decision_payload: None,
            orchestration_audit_payload: None,
            delivery_payload: None,
            generator_service_response: None,
            storage_path: None,
            file_url: None,
            thumbnail_url: None,
            mime_type: None,
            error_code: None,
            error_message: None,
            generation_job_id: None,
            generation_status: None,
            s3_object_key: None,
            presigned_download_url: None,
            presigned_url_expires_at: None,
            generation_error_code: None,
            generation_error_message: None,
            clarification_state: Some(serde_json::json!({
                "answers": {"target_audience": "SD_Kelas_5"},
                "suggested_prompt": "Buatkan materi pecahan untuk SD Kelas 5",
            })),
            clarified_at: Some(chrono::Utc::now()),
            clarification_skipped: false,
            created_at: None,
            updated_at: None,
        };

        assert!(gen.clarification_state.is_some());
        assert!(gen.clarified_at.is_some());
        assert!(!gen.clarification_skipped);
    }
}
