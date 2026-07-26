-- ══════════════════════════════════════════════════════════════════════════════
-- ADD FREELANCER FIELDS TO USERS TABLE
-- Run this in: Neon Dashboard → SQL Editor
-- ══════════════════════════════════════════════════════════════════════════════

ALTER TABLE users
ADD COLUMN IF NOT EXISTS rating NUMERIC(3,2) DEFAULT 4.8,
ADD COLUMN IF NOT EXISTS job_count INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS hourly_rate VARCHAR(50) DEFAULT '120K',
ADD COLUMN IF NOT EXISTS skills JSONB DEFAULT '[]'::jsonb,
ADD COLUMN IF NOT EXISTS response_time VARCHAR(50) DEFAULT '< 30m';

-- Update values for each freelancer
UPDATE users
SET
    rating = 4.9,
    job_count = 28,
    hourly_rate = '150K',
    skills = '["Curriculum Design", "PPT Specialist", "Matematika"]'::jsonb,
    response_time = '< 15m'
WHERE name = 'Agus Pratama';

UPDATE users
SET
    rating = 4.8,
    job_count = 19,
    hourly_rate = '120K',
    skills = '["Infografis Pembelajaran", "Biologi", "Bank Soal"]'::jsonb,
    response_time = '< 30m'
WHERE name = 'Ani Wulandari';

UPDATE users
SET
    rating = 5.0,
    job_count = 34,
    hourly_rate = '200K',
    skills = '["Modul Ajar Kurikulum Merdeka", "Fisika", "Media Interaktif"]'::jsonb,
    response_time = '< 10m'
WHERE name = 'Budi Santoso';

UPDATE users
SET
    rating = 4.7,
    job_count = 15,
    hourly_rate = '100K',
    skills = '["Bahasa Indonesia", "Lembar Kerja Siswa", "RPP Plus"]'::jsonb,
    response_time = '< 1h'
WHERE name = 'Susi Rahmawati';
