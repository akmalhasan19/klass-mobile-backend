-- Seed recommended_projects with 4 PPT placeholder projects
-- Thumbnail images are hosted in R2 bucket: klass-storage/assets/
-- FIX: explicitly include created_at/updated_at to avoid NULL

INSERT INTO recommended_projects (title, description, thumbnail_url, ratio, source_type, display_priority, is_active, created_at, updated_at)
VALUES
    (
        'Presentasi Geologi',
        'Template presentasi PPT untuk mata pelajaran Geologi',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/klass-storage/assets/ppt_geologi.jpg',
        'ppt',
        'admin_upload',
        100,
        true,
        NOW(),
        NOW()
    ),
    (
        'Presentasi Biologi',
        'Template presentasi PPT untuk mata pelajaran Biologi',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/klass-storage/assets/ppt_biologi.jpg',
        'ppt',
        'admin_upload',
        90,
        true,
        NOW(),
        NOW()
    ),
    (
        'Presentasi Kalkulus',
        'Template presentasi PPT untuk mata pelajaran Kalkulus',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/klass-storage/assets/ppt_kalkulus.jpg',
        'ppt',
        'admin_upload',
        80,
        true,
        NOW(),
        NOW()
    ),
    (
        'Presentasi Pendidikan Pancasila',
        'Template presentasi PPT untuk mata pelajaran Pendidikan Pancasila',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/klass-storage/assets/ppt_pancasila.jpg',
        'ppt',
        'admin_upload',
        70,
        true,
        NOW(),
        NOW()
    );

-- Ensure homepage section 'project_recommendations' is enabled
INSERT INTO homepage_sections (key, label, position, is_enabled)
VALUES ('project_recommendations', 'Rekomendasi Project', 1, true)
ON CONFLICT (key) DO UPDATE SET is_enabled = true;
