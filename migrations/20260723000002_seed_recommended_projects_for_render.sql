-- ══════════════════════════════════════════════════════════════════════════════
-- SEED RECOMMENDED PROJECTS WITH R2 THUMBNAILS
-- Run this in: Render Dashboard → PostgreSQL → Query tab
-- Uses ON CONFLICT so it's safe to run even if data already exists.
-- ══════════════════════════════════════════════════════════════════════════════

-- Seed recommended_projects with 4 PPT placeholder projects
-- Thumbnail images: https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/klass-storage/assets/
INSERT INTO recommended_projects (title, description, thumbnail_url, ratio, source_type, display_priority, is_active)
VALUES
    (
        'Presentasi Geologi',
        'Template presentasi PPT untuk mata pelajaran Geologi',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/klass-storage/assets/ppt_geologi.jpg',
        'ppt',
        'admin_upload',
        100,
        true
    ),
    (
        'Presentasi Biologi',
        'Template presentasi PPT untuk mata pelajaran Biologi',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/klass-storage/assets/ppt_biologi.jpg',
        'ppt',
        'admin_upload',
        90,
        true
    ),
    (
        'Presentasi Kalkulus',
        'Template presentasi PPT untuk mata pelajaran Kalkulus',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/klass-storage/assets/ppt_kalkulus.jpg',
        'ppt',
        'admin_upload',
        80,
        true
    ),
    (
        'Presentasi Pendidikan Pancasila',
        'Template presentasi PPT untuk mata pelajaran Pendidikan Pancasila',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/klass-storage/assets/ppt_pancasila.jpg',
        'ppt',
        'admin_upload',
        70,
        true
    );

-- Ensure homepage section 'project_recommendations' is enabled
INSERT INTO homepage_sections (key, label, position, is_enabled)
VALUES ('project_recommendations', 'Rekomendasi Project', 1, true)
ON CONFLICT (key) DO UPDATE SET is_enabled = true;

-- ══════════════════════════════════════════════════════════════════════════════
-- DONE! After running this, refresh the app to see the projects.
-- ══════════════════════════════════════════════════════════════════════════════
