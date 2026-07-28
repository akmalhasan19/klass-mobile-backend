-- ══════════════════════════════════════════════════════════════════════════════
-- ADD FREELANCER PROFILE SUPPORT
-- - Adds bio & verified columns to users table (for freelancer profiles)
-- - Creates freelancer_portfolio_items table
-- ══════════════════════════════════════════════════════════════════════════════

-- Add profile fields to users table
ALTER TABLE users
ADD COLUMN IF NOT EXISTS bio TEXT NOT NULL DEFAULT '',
ADD COLUMN IF NOT EXISTS location VARCHAR(255) NOT NULL DEFAULT '',
ADD COLUMN IF NOT EXISTS verified BOOLEAN NOT NULL DEFAULT FALSE;

-- Portfolio items for freelancers
CREATE TABLE IF NOT EXISTS freelancer_portfolio_items (
    id BIGSERIAL PRIMARY KEY,
    freelancer_user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    image_url TEXT NULL,
    category VARCHAR(100) NOT NULL DEFAULT '',
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_freelancer_portfolio_items_user
    ON freelancer_portfolio_items (freelancer_user_id, sort_order);

-- Seed some portfolio items for existing freelancers
DO $$
DECLARE
    agus_id BIGINT;
    ani_id BIGINT;
    budi_id BIGINT;
    susi_id BIGINT;
BEGIN
    SELECT id INTO agus_id FROM users WHERE name = 'Agus Pratama';
    SELECT id INTO ani_id FROM users WHERE name = 'Ani Wulandari';
    SELECT id INTO budi_id FROM users WHERE name = 'Budi Santoso';
    SELECT id INTO susi_id FROM users WHERE name = 'Susi Rahmawati';

    -- Update bio/location/verified for each freelancer
    UPDATE users SET bio = 'Guru matematika berpengalaman dengan spesialisasi pembuatan PPT interaktif dan kurikulum merdeka.', location = 'Jakarta', verified = TRUE WHERE id = agus_id;
    UPDATE users SET bio = 'Ahli infografis pembelajaran dan materi biologi untuk SMA/SMK sederajat.', location = 'Bandung', verified = TRUE WHERE id = ani_id;
    UPDATE users SET bio = 'Kreator modul ajar dan media interaktif fisika terbaik. Sudah membuat 30+ modul.', location = 'Yogyakarta', verified = TRUE WHERE id = budi_id;
    UPDATE users SET bio = 'Spesialis bahasa Indonesia dan pembuat lembar kerja siswa yang kreatif dan menarik.', location = 'Surabaya', verified = TRUE WHERE id = susi_id;

    -- Portfolio for Agus
    INSERT INTO freelancer_portfolio_items (freelancer_user_id, title, description, image_url, category, sort_order)
    VALUES
        (agus_id, 'PPT Matematika Kelas 10 - Semester 1', 'PPT interaktif lengkap dengan animasi dan latihan soal untuk semester 1.', NULL, 'PPT', 1),
        (agus_id, 'Bank Soal Trigonometri', 'Kumpulan 100+ soal trigonometri dengan pembahasan detail.', NULL, 'Bank Soal', 2),
        (agus_id, 'Modul Ajar Matriks', 'Modul ajar matriks lengkap dengan RPP dan LKPD.', NULL, 'Modul Ajar', 3);

    -- Portfolio for Ani
    INSERT INTO freelancer_portfolio_items (freelancer_user_id, title, description, image_url, category, sort_order)
    VALUES
        (ani_id, 'Infografis Sistem Peredaran Darah', 'Infografis digital tentang sistem peredaran darah manusia.', NULL, 'Infografis', 1),
        (ani_id, 'LKS Biologi Kelas 11', 'Lembar kerja siswa biologi untuk semester genap.', NULL, 'LKS', 2);

    -- Portfolio for Budi
    INSERT INTO freelancer_portfolio_items (freelancer_user_id, title, description, image_url, category, sort_order)
    VALUES
        (budi_id, 'Modul Interaktif Fisika - Gerak Lurus', 'Modul interaktif HTML5 tentang gerak lurus beraturan dan berubah.', NULL, 'Media Interaktif', 1),
        (budi_id, 'RPP Kurikulum Merdeka Fisika', 'RPP fisika SMA kurikulum merdeka lengkap dengan asesmen.', NULL, 'RPP', 2),
        (budi_id, 'Video Animasi Hukum Newton', 'Video animasi 3D menjelaskan 3 hukum Newton.', NULL, 'Video', 3);

    -- Portfolio for Susi
    INSERT INTO freelancer_portfolio_items (freelancer_user_id, title, description, image_url, category, sort_order)
    VALUES
        (susi_id, 'LKPD Bahasa Indonesia Kelas 12', 'Lembar kerja peserta didik untuk teks cerita sejarah.', NULL, 'LKPD', 1),
        (susi_id, 'Infografis Kaidah Kebahasaan', 'Infografis tentang kaidah kebahasaan teks prosedur.', NULL, 'Infografis', 2);
END $$;
