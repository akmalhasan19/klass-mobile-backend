//! Seed module for subjects and sub_subjects.
//!
//! On startup, checks if the `subjects` and `sub_subjects` tables have data.
//! If empty, seeds them from the embedded `subjects.json` taxonomy data.
//!
//! This uses `ON CONFLICT DO NOTHING` for idempotency — safe to run multiple times.

use serde::Deserialize;
use sqlx::PgPool;

// ─── Embedded taxonomy data ──────────────────────────────────────────────────

/// Raw entry from `subjects.json`.
#[derive(Debug, Deserialize)]
struct SubjectJsonEntry {
    #[serde(default)]
    jenjang: Option<String>,
    subject: Option<String>,
    subject_slug: Option<String>,
    kelas: Option<i64>,
    semester: Option<i64>,
    bab: Option<i64>,
    sub_subject: Option<String>,
    sub_subject_slug: Option<String>,
    #[serde(default)]
    deskripsi_singkat: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "return_true")]
    #[allow(dead_code)]
    is_active: bool,
}

fn return_true() -> bool {
    true
}

// ─── Extracted types ─────────────────────────────────────────────────────────

/// A unique subject extracted from the taxonomy.
#[derive(Debug, Clone)]
struct SeedSubject {
    name: String,
    slug: String,
    description: String,
    #[allow(dead_code)]
    jenjang: String,
}

/// A sub_subject associated with a subject.
#[derive(Debug, Clone)]
struct SeedSubSubject {
    subject_slug: String,
    name: String,
    slug: String,
    description: String,
    kelas: i64,
    semester: i64,
    bab: i64,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Check if the `subjects` table already has data, and seed if empty.
///
/// This is designed to be called once at application startup (from
/// `AppState::new()`). It acquires an advisory lock to prevent concurrent
/// seeding in multi-instance deployments.
pub async fn seed_if_empty(pool: &PgPool) -> anyhow::Result<()> {
    // Advisory lock to prevent concurrent seeding
    let lock_result: Result<(bool,), _> = sqlx::query_as(
        "SELECT pg_try_advisory_lock(20260712000016)",
    )
    .fetch_one(pool)
    .await;

    let has_lock = match lock_result {
        Ok((true,)) => true,
        _ => {
            tracing::info!(
                "seed: another instance is already seeding subjects, skipping"
            );
            return Ok(());
        }
    };

    let result = try_seed(pool).await;

    // Release the lock
    if has_lock {
        let _ = sqlx::query("SELECT pg_advisory_unlock(20260712000016)")
            .execute(pool)
            .await;
    }

    result
}

async fn try_seed(pool: &PgPool) -> anyhow::Result<()> {
    // Check if subjects already have data
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM subjects")
        .fetch_one(pool)
        .await?;

    if count.0 > 0 {
        tracing::info!(
            "seed: subjects table already has {} rows, skipping",
            count.0
        );
        return Ok(());
    }

    tracing::info!(
        "seed: subjects table is empty — seeding from taxonomy data"
    );

    // Parse the embedded JSON
    let raw: Vec<SubjectJsonEntry> =
        serde_json::from_str(include_str!("../recommendation/data/subjects.json"))
            .map_err(|e| anyhow::anyhow!("failed to parse subjects.json: {e}"))?;

    // Extract unique subjects
    let (subjects, sub_subjects) = extract_subjects_and_sub_subjects(&raw);

    if subjects.is_empty() {
        tracing::warn!("seed: no valid subjects found in taxonomy data");
        return Ok(());
    }

    // Use a transaction for atomicity
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| anyhow::anyhow!("failed to begin seed transaction: {e}"))?;

    // Insert subjects
    let mut subject_id_map: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();

    for (idx, subject) in subjects.iter().enumerate() {
        let display_order = (idx + 1) as i32;

        let row: Option<(i64,)> = sqlx::query_as(
            r#"
            INSERT INTO subjects (name, slug, description, display_order, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, TRUE, NOW(), NOW())
            ON CONFLICT (slug) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(&subject.name)
        .bind(&subject.slug)
        .bind(&subject.description)
        .bind(display_order)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| anyhow::anyhow!("failed to insert subject '{}': {e}", subject.name))?;

        if let Some((id,)) = row {
            subject_id_map.insert(subject.slug.clone(), id);
        }
    }

    // Log how many subjects were inserted
    let inserted_count = subject_id_map.len();
    tracing::info!("seed: inserted {} subjects", inserted_count);

    if sub_subjects.is_empty() {
        tracing::warn!("seed: no sub_subjects found to insert");
        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("failed to commit seed: {e}"))?;
        return Ok(());
    }

    // Insert sub_subjects in batches
    let mut sub_subject_inserted = 0usize;
    for sub in &sub_subjects {
        let subject_id = match subject_id_map.get(&sub.subject_slug) {
            Some(id) => *id,
            None => {
                tracing::warn!(
                    "seed: skipping sub_subject '{}' — subject '{}' not found",
                    sub.name,
                    sub.subject_slug
                );
                continue;
            }
        };

        let result = sqlx::query(
            r#"
            INSERT INTO sub_subjects
                (subject_id, name, slug, description, display_order, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, TRUE, NOW(), NOW())
            ON CONFLICT (subject_id, slug) DO NOTHING
            "#,
        )
        .bind(subject_id)
        .bind(&sub.name)
        .bind(&sub.slug)
        .bind(&sub.description)
        .bind(compute_sub_subject_display_order(sub))
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow::anyhow!("failed to insert sub_subject '{}': {e}", sub.name))?;

        sub_subject_inserted += result.rows_affected() as usize;
    }

    tx.commit()
        .await
        .map_err(|e| anyhow::anyhow!("failed to commit seed transaction: {e}"))?;

    tracing::info!(
        "seed: completed — {} subjects, {} sub_subjects",
        inserted_count,
        sub_subject_inserted
    );

    Ok(())
}

// ─── Extraction ──────────────────────────────────────────────────────────────

/// Extract unique subjects and their sub_subjects from the parsed JSON.
fn extract_subjects_and_sub_subjects(
    entries: &[SubjectJsonEntry],
) -> (Vec<SeedSubject>, Vec<SeedSubSubject>) {
    let mut subject_map: std::collections::BTreeMap<String, SeedSubject> =
        std::collections::BTreeMap::new();
    let mut sub_subjects: Vec<SeedSubSubject> = Vec::new();

    for entry in entries {
        let subject_name = match entry.subject.as_ref() {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => continue,
        };
        let subject_slug = match entry.subject_slug.as_ref() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => slugify(&subject_name),
        };
        let sub_subject_name = match entry.sub_subject.as_ref() {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => continue,
        };
        let sub_subject_slug = match entry.sub_subject_slug.as_ref() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => slugify(&sub_subject_name),
        };

        let description = entry
            .deskripsi_singkat
            .as_deref()
            .or(entry.description.as_deref())
            .unwrap_or("")
            .to_string();

        let jenjang = entry
            .jenjang
            .as_deref()
            .unwrap_or("")
            .to_string();

        // Insert subject (using BTreeMap for deterministic ordering by slug)
        subject_map.entry(subject_slug.clone()).or_insert_with(|| SeedSubject {
            name: subject_name.clone(),
            slug: subject_slug.clone(),
            description: format!("{} — Kurikulum Merdeka {}", subject_name, jenjang),
            jenjang,
        });

        // Add sub_subject
        sub_subjects.push(SeedSubSubject {
            subject_slug,
            name: sub_subject_name,
            slug: sub_subject_slug,
            description,
            kelas: entry.kelas.unwrap_or(0),
            semester: entry.semester.unwrap_or(0),
            bab: entry.bab.unwrap_or(0),
        });
    }

    let subjects: Vec<SeedSubject> = subject_map.into_values().collect();

    (subjects, sub_subjects)
}

/// Compute a display_order for a sub_subject based on kelas, semester, bab.
fn compute_sub_subject_display_order(sub: &SeedSubSubject) -> i32 {
    // Order by kelas → semester → bab
    (sub.kelas as i32) * 10000 + (sub.semester as i32) * 100 + (sub.bab as i32)
}

/// Simple slugify helper.
fn slugify(value: &str) -> String {
    let re = regex::Regex::new(r"[^a-zA-Z0-9]+").unwrap();
    let slug = re.replace_all(value, "-");
    let slug = slug.trim_matches('-');
    slug.to_lowercase()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_subjects_empty() {
        let (subjects, sub_subjects) = extract_subjects_and_sub_subjects(&[]);
        assert!(subjects.is_empty());
        assert!(sub_subjects.is_empty());
    }

    #[test]
    fn test_extract_subjects_single_entry() {
        let entries = vec![SubjectJsonEntry {
            jenjang: Some("SD".to_string()),
            subject: Some("Mathematics".to_string()),
            subject_slug: Some("mathematics-sd".to_string()),
            kelas: Some(1),
            semester: Some(1),
            bab: Some(1),
            sub_subject: Some("Addition".to_string()),
            sub_subject_slug: Some("addition".to_string()),
            deskripsi_singkat: Some("Learn addition".to_string()),
            description: None,
            is_active: true,
        }];

        let (subjects, sub_subjects) = extract_subjects_and_sub_subjects(&entries);
        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0].name, "Mathematics");
        assert_eq!(subjects[0].slug, "mathematics-sd");

        assert_eq!(sub_subjects.len(), 1);
        assert_eq!(sub_subjects[0].name, "Addition");
        assert_eq!(sub_subjects[0].subject_slug, "mathematics-sd");
    }

    #[test]
    fn test_extract_subjects_deduplicates() {
        let entries = vec![
            SubjectJsonEntry {
                jenjang: Some("SD".to_string()),
                subject: Some("Math".to_string()),
                subject_slug: Some("math-sd".to_string()),
                kelas: Some(1),
                semester: Some(1),
                bab: Some(1),
                sub_subject: Some("Addition".to_string()),
                sub_subject_slug: Some("addition".to_string()),
                deskripsi_singkat: None,
                description: None,
                is_active: true,
            },
            SubjectJsonEntry {
                jenjang: Some("SD".to_string()),
                subject: Some("Math".to_string()),
                subject_slug: Some("math-sd".to_string()),
                kelas: Some(1),
                semester: Some(2),
                bab: Some(2),
                sub_subject: Some("Subtraction".to_string()),
                sub_subject_slug: Some("subtraction".to_string()),
                deskripsi_singkat: None,
                description: None,
                is_active: true,
            },
        ];

        let (subjects, sub_subjects) = extract_subjects_and_sub_subjects(&entries);
        assert_eq!(subjects.len(), 1, "should deduplicate subjects");
        assert_eq!(sub_subjects.len(), 2, "should keep both sub_subjects");
    }

    #[test]
    fn test_extract_skips_invalid_entries() {
        let entries = vec![
            SubjectJsonEntry {
                jenjang: None,
                subject: None,
                subject_slug: None,
                kelas: None,
                semester: None,
                bab: None,
                sub_subject: Some("Only sub".to_string()),
                sub_subject_slug: None,
                deskripsi_singkat: None,
                description: None,
                is_active: true,
            },
            SubjectJsonEntry {
                jenjang: None,
                subject: Some("Subject".to_string()),
                subject_slug: None,
                kelas: None,
                semester: None,
                bab: None,
                sub_subject: None,
                sub_subject_slug: None,
                deskripsi_singkat: None,
                description: None,
                is_active: true,
            },
        ];

        let (subjects, sub_subjects) = extract_subjects_and_sub_subjects(&entries);
        assert!(subjects.is_empty(), "no valid subjects");
        assert!(sub_subjects.is_empty(), "no valid sub_subjects");
    }

    #[test]
    fn test_compute_display_order() {
        let sub = SeedSubSubject {
            subject_slug: "math".to_string(),
            name: "Test".to_string(),
            slug: "test".to_string(),
            description: "".to_string(),
            kelas: 5,
            semester: 2,
            bab: 3,
        };
        let order = compute_sub_subject_display_order(&sub);
        assert_eq!(order, 50203); // kelas*10000 + semester*100 + bab
    }

    #[test]
    fn test_entries_loaded_from_embedded_json() {
        let raw: Vec<SubjectJsonEntry> =
            serde_json::from_str(include_str!("../recommendation/data/subjects.json"))
                .expect("embedded subjects.json must be valid");
        assert!(!raw.is_empty(), "subjects.json should have entries");

        let (subjects, sub_subjects) = extract_subjects_and_sub_subjects(&raw);
        assert!(!subjects.is_empty(), "should extract at least one subject");
        assert!(!sub_subjects.is_empty(), "should extract at least one sub_subject");

        // Verify subject structure
        let first = &subjects[0];
        assert!(!first.name.is_empty());
        assert!(!first.slug.is_empty());
    }

    #[test]
    fn test_subject_slug_uniqueness() {
        let raw: Vec<SubjectJsonEntry> =
            serde_json::from_str(include_str!("../recommendation/data/subjects.json"))
                .expect("valid JSON");
        let (subjects, _) = extract_subjects_and_sub_subjects(&raw);

        let mut slugs: std::collections::HashSet<String> = std::collections::HashSet::new();
        for s in &subjects {
            assert!(
                slugs.insert(s.slug.clone()),
                "duplicate subject slug: {}",
                s.slug
            );
        }
    }
}
