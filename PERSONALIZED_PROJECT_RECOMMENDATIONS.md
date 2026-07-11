# Personalized Project Recommendations

This document is the backend runbook for the personalized homepage recommendation flow and the admin-facing system distribution summary.

## Scope

- Mobile feed endpoint: `GET /api/homepage-recommendations`
- Admin configurator: `/admin/homepage-sections`
- Policy config: `config/personalized_project_recommendations.php`
- Personalized system sources: `ai_generated` and `system_topic`
- Always-visible curated source: `admin_upload`

## Runtime Flow

### 1. Homepage section gate

The recommendation feed is only active when `homepage_sections.key = project_recommendations` is enabled. If the section is disabled, the API returns an empty collection with section metadata and `not_evaluated` source states.

### 2. Audience split

- Guest requests stay on the safe global feed.
- Authenticated requests resolve personalization context before selecting system-generated candidates.

### 3. Personalization signals

The backend resolves personalization in this order:

1. `users.primary_subject_id` as the main subject anchor.
2. Authored-topic activity from `topics.owner_user_id`, grouped by `topics.sub_subject_id`, ranked by topic count and latest topic activity.
3. If a primary subject exists, authored activity inside that subject is preferred before activity from other subjects.

Curated admin uploads are not filtered out by personalization. They stay visible as long as the section is enabled and each row remains visible by its own scheduling and activation rules.

### 4. Candidate selection and feed composition

- The feed starts from visible curated rows in `recommended_projects`.
- Admin-curated items are separated from system-generated items.
- System-generated items come from persisted `recommended_projects` rows with non-admin sources plus normalized published topics.
- Suppressed topic duplicates stay hidden when a persisted non-admin override already exists for the same source.
- For authenticated users with enough signals, system-generated items are filtered and ordered by matched sub-subject relevance.
- For guests or authenticated users without enough signals, the API falls back to the safe global feed.

### 5. Assignment tracking

When an authenticated response includes eligible system-generated items, the backend upserts rows into `system_recommendation_assignments`.

- Tracking is deduplicated by `user_id + recommendation_key`.
- Repeated refreshes update `last_distributed_at` without inflating distinct-user counts.
- Guest requests do not create assignment rows.
- Tracking failures are swallowed so the homepage response still succeeds.

## Schema Reference

### `subjects`

Purpose: top-level personalization subject anchor and taxonomy parent for admin aggregation.

Important fields:

- `id`: primary key used by users, sub-subjects, topics, and assignments.
- `name`: human-readable label shown in admin summary payloads.
- `slug`: stable identifier used in tests and serialized API payloads.
- `description`: optional descriptive metadata.
- `display_order`: seed ordering for deterministic baseline taxonomy.
- `is_active`: soft availability flag for future management.

### `sub_subjects`

Purpose: concrete taxonomy bucket for topic ownership activity, system recommendation matching, and admin summary grouping.

Important fields:

- `id`: primary key used by topics and assignment summary rows.
- `subject_id`: parent subject relation.
- `name`: human-readable label shown in admin summary payloads.
- `slug`: stable identifier for API payloads and tests.
- `description`: optional descriptive metadata.
- `display_order`: seed ordering within a subject.
- `is_active`: soft availability flag for future management.

### `topics`

Purpose: published topic catalog, authored-topic signal source, and raw fallback source for system-generated recommendations.

Relevant fields for this feature:

- `teacher_id`: legacy identifier kept for backward compatibility.
- `owner_user_id`: normalized user ownership anchor used for personalization.
- `ownership_status`: normalization status such as `normalized` or `legacy_unresolved`.
- `sub_subject_id`: taxonomy anchor used for personalization and subject derivation.

Behavioral notes:

- `teacher_id` still accepts legacy identifiers for existing clients.
- `sub_subject_id` is now the only explicit taxonomy pointer on topics.
- `subject_id` is derived through the related sub-subject and is not duplicated on the table.
- Topics missing normalized ownership or missing `sub_subject_id` are excluded from personalization signals.

### `users`

Purpose: profile-level personalization anchor.

Relevant field:

- `primary_subject_id`: optional subject chosen as the first personalization signal.

Behavioral note: if this field is null, the system falls back to authored-topic activity when available.

### `system_recommendation_assignments`

Purpose: persistent distinct-user distribution log for authenticated system recommendations.

Important fields:

- `user_id`: authenticated recipient of the served recommendation.
- `recommendation_key`: stable dedupe key in the format `source_type:source_reference`.
- `recommendation_item_id`: item ID returned in the feed payload.
- `source_type`: recommendation source, currently `ai_generated` or `system_topic`.
- `source_reference`: original source identifier used for aggregation.
- `subject_id`: resolved subject context at distribution time.
- `sub_subject_id`: resolved sub-subject context at distribution time.
- `first_distributed_at`: first time the user received the item.
- `last_distributed_at`: most recent time the user received the item.

Operational note: `user_id + recommendation_key` is unique, so repeated refreshes update the same row instead of creating duplicates.

## Fallback and Guardrail Policy

### Guest behavior

- Mode: `default_global_feed`
- Tracking: disabled
- Description: guests keep the non-personalized homepage feed until an authenticated context exists.

### Authenticated users without enough signals

- Mode: `default_global_feed`
- Tracking: enabled
- Description: the API serves the current safe mixed feed while subject profile or authored-topic signals are insufficient.

### Topic guardrails

- Topics without `sub_subject_id` are `general_feed_only` and cannot influence personalization ranking.
- Topics with unresolved ownership stay eligible for the general feed only when configured, but they are excluded from personalization signals.
- Curated admin uploads do not depend on these guardrails and continue to render when visible.

## Admin Aggregation Rules

The admin summary section `Top Distributed System Recommendations by Sub-Subject` is read-only and does not change curated CRUD behavior.

### Eligibility

- Only `system_topic` and `ai_generated` assignments are counted.
- Only rows with a non-null `source_reference` and non-null `sub_subject_id` are eligible.
- Each candidate is grouped by `source_type + source_reference + subject_id + sub_subject_id`.
- The candidate must have `COUNT(DISTINCT user_id) >= 2`.

### Selection

- Group candidates by sub-subject.
- Select at most one winning item per sub-subject.
- Resolve subject and sub-subject labels from taxonomy tables.

### Tie-breakers

Tie-breakers run in this order:

1. `distinct_user_count` descending
2. `latest_distribution_at` descending
3. `source_created_at` descending
4. `source_reference` ascending

If the configured fields still tie, the service falls back to source type and title for stable ordering.

### Title resolution

- `ai_generated`: title comes from the persisted `recommended_projects` row.
- `system_topic` with override: title comes from the persisted override row.
- `system_topic` without override: title falls back to the raw `topics` row.

### Empty state

If no candidate passes the minimum distinct-user threshold, the admin UI shows:

`No system recommendation has been distributed to more than one user yet.`

## Seed and Backfill Operations

### Fresh environments

For a fresh environment, the simplest path is:

```bash
php artisan migrate
php artisan db:seed
```

`DatabaseSeeder` already includes `SubjectTaxonomySeeder`, so baseline taxonomy is available after the standard seed flow.

### Existing environments

If the database already contains legacy topics, deploy in this order:

```bash
php artisan migrate
php artisan db:seed --class=SubjectTaxonomySeeder
php artisan app:backfill-topic-ownership
```

Why this order matters:

- migrations add `topics.owner_user_id`, `topics.ownership_status`, `topics.sub_subject_id`, `users.primary_subject_id`, and `system_recommendation_assignments`
- taxonomy must exist before operators start assigning `sub_subject_id` or `primary_subject_id`
- ownership backfill should run after the relevant user rows are already present

### Backfill command notes

Command:

```bash
php artisan app:backfill-topic-ownership
```

Behavior:

- default option `--only-unresolved=1` scans only topics that are not yet normalized
- numeric `teacher_id` values are matched against `users.id`
- email-like `teacher_id` values are matched case-insensitively against `users.email`
- unmatched rows remain `legacy_unresolved`

To rescan all topics intentionally:

```bash
php artisan app:backfill-topic-ownership --only-unresolved=0
```

### Post-deploy smoke checks

After deployment, verify these flows:

1. Guest `GET /api/homepage-recommendations` still returns a safe feed.
2. Authenticated `GET /api/homepage-recommendations` reflects subject and authored-topic relevance when signals exist.
3. Repeated authenticated refreshes do not create duplicate assignment rows.
4. Admin Homepage Configurator shows curated projects and the system distribution summary as separate sections.
5. Curated project CRUD still behaves normally, including thumbnail uploads.

## Useful Test Commands

```bash
php artisan test --filter=HomepageRecommendationApiTest
php artisan test --filter=RecommendationAssignmentTrackingTest
php artisan test --filter=RecommendationAggregationServiceTest
php artisan test --filter=AdminHomepageSectionConfigurationTest
php artisan test --filter=Phase7EndToEndVerificationTest
```