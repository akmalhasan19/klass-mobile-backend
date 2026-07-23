-- ══════════════════════════════════════════════════════════════════════════════
-- FIX: Update thumbnail URLs to remove bucket name from path
-- Run this in: Neon Dashboard → SQL Editor
-- ══════════════════════════════════════════════════════════════════════════════

-- Fix: Update all 4 recommended projects with correct R2 URLs (no bucket name in path)
UPDATE recommended_projects
SET thumbnail_url = 'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/assets/ppt_geologi.jpg'
WHERE title = 'Presentasi Geologi';

UPDATE recommended_projects
SET thumbnail_url = 'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/assets/ppt_biologi.jpg'
WHERE title = 'Presentasi Biologi';

UPDATE recommended_projects
SET thumbnail_url = 'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/assets/ppt_kalkulus.jpg'
WHERE title = 'Presentasi Kalkulus';

UPDATE recommended_projects
SET thumbnail_url = 'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/assets/ppt_pancasila.jpg'
WHERE title = 'Presentasi Pendidikan Pancasila';

-- Verify the updated URLs
SELECT title, thumbnail_url FROM recommended_projects WHERE source_type = 'admin_upload';
