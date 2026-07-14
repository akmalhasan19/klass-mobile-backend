-- Drop CHECK constraint on marketplace_tasks.status to allow new status values
-- (e.g. 'assigned', 'open_for_bid') needed by the freelancer matching workflow in Phase 7.7.
--
-- The column is already VARCHAR(20), so no ALTER COLUMN type change is needed.
-- Validation is handled at the application layer (see VALID_STATUSES in
-- src/api/rest/admin/marketplace_tasks.rs).

ALTER TABLE marketplace_tasks DROP CONSTRAINT IF EXISTS marketplace_tasks_status_check;
