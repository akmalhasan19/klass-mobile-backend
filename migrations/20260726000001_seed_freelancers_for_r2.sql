INSERT INTO users (name, email, password, role, avatar_url, created_at, updated_at)
VALUES
    (
        'Agus Pratama',
        'agus.pratama@klass.id',
        '$2b$12$e8x8rJ4y/v9123456789012345678901234567890123456789012',
        'freelancer',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/assets/agus_real.jpg',
        NOW(),
        NOW()
    ),
    (
        'Ani Wulandari',
        'ani.wulandari@klass.id',
        '$2b$12$e8x8rJ4y/v9123456789012345678901234567890123456789012',
        'freelancer',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/assets/ani_real.jpg',
        NOW(),
        NOW()
    ),
    (
        'Budi Santoso',
        'budi.santoso@klass.id',
        '$2b$12$e8x8rJ4y/v9123456789012345678901234567890123456789012',
        'freelancer',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/assets/budi_real.jpg',
        NOW(),
        NOW()
    ),
    (
        'Susi Rahmawati',
        'susi.rahmawati@klass.id',
        '$2b$12$e8x8rJ4y/v9123456789012345678901234567890123456789012',
        'freelancer',
        'https://pub-7ec094e10eed491fb2160f17e582f8bf.r2.dev/assets/susi_real.jpg',
        NOW(),
        NOW()
    )
ON CONFLICT (email) DO UPDATE SET
    name = EXCLUDED.name,
    avatar_url = EXCLUDED.avatar_url,
    role = 'freelancer',
    updated_at = NOW();
