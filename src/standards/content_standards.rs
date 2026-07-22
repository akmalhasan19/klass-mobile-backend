//! Content standards per media type.
//!
//! Defines which fields are required, recommended, or optional for each
//! content type (materi pembelajaran, slide presentasi, RPP, lembar kerja,
//! silabus, penilaian). Also defines suggestion chips for each field.

// ─── Types ──────────────────────────────────────────────────────────────────

/// Priority of a field in the clarification flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldPriority {
    Required,
    Recommended,
    Optional,
}

impl FieldPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldPriority::Required => "required",
            FieldPriority::Recommended => "recommended",
            FieldPriority::Optional => "optional",
        }
    }
}

/// The type of input widget for a clarification question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputType {
    Select,
    MultiSelect,
    TextInput,
    NumberInput,
}

impl InputType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InputType::Select => "select",
            InputType::MultiSelect => "multi_select",
            InputType::TextInput => "text_input",
            InputType::NumberInput => "number_input",
        }
    }
}

/// A suggestion chip shown alongside a clarification question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionChip {
    pub value: String,
    pub label: String,
}

/// Definition of a single field in the content standards.
#[derive(Debug, Clone)]
pub struct FieldDefinition {
    pub field_id: &'static str,
    pub label_id: &'static str,
    pub label_en: &'static str,
    pub input_type: InputType,
    pub priority: FieldPriority,
    pub suggestions: Vec<SuggestionChip>,
}

/// Content type classification result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    MateriPembelajaran,
    SlidePresentasi,
    Rpp,
    LembarKerja,
    Silabus,
    Penilaian,
    Unknown,
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::MateriPembelajaran => "materi_pembelajaran",
            ContentType::SlidePresentasi => "slide_presentasi",
            ContentType::Rpp => "rpp",
            ContentType::LembarKerja => "lembar_kerja",
            ContentType::Silabus => "silabus",
            ContentType::Penilaian => "penilaian",
            ContentType::Unknown => "unknown",
        }
    }
}

/// A gap: a field that needs clarification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentGap {
    pub field_id: String,
    pub question: String,
    pub priority: String,
    pub input_type: String,
    pub suggestions: Vec<SuggestionChip>,
    pub detected_value: Option<String>,
}

// ─── Standards registry ───────────────────────────────────────────────────

/// Get the standards for a given content type.
///
/// Returns the list of field definitions ordered by priority (required first,
/// then recommended, then optional). Max returned = 5 (matching the max
/// questions constraint).
pub fn get_standards_for_content_type(content_type: &ContentType) -> Vec<FieldDefinition> {
    match content_type {
        ContentType::MateriPembelajaran => materi_pembelajaran_standards(),
        ContentType::SlidePresentasi => slide_presentasi_standards(),
        ContentType::Rpp => rpp_standards(),
        ContentType::LembarKerja => lembar_kerja_standards(),
        ContentType::Silabus => silabus_standards(),
        ContentType::Penilaian => penilaian_standards(),
        ContentType::Unknown => default_standards(),
    }
}

/// Get only required + recommended fields (max 5), ordered by priority.
pub fn get_clarification_fields(content_type: &ContentType) -> Vec<FieldDefinition> {
    let all = get_standards_for_content_type(content_type);
    let mut filtered: Vec<_> = all
        .into_iter()
        .filter(|f| f.priority != FieldPriority::Optional)
        .take(5)
        .collect();

    // Sort: required first, then recommended
    filtered.sort_by_key(|f| match f.priority {
        FieldPriority::Required => 0,
        FieldPriority::Recommended => 1,
        FieldPriority::Optional => 2,
    });
    filtered
}

// ─── Content type detection ──────────────────────────────────────────────

/// Detect content type from a raw prompt using keyword signals.
pub fn detect_content_type(prompt: &str) -> (ContentType, f64) {
    let lower = prompt.to_lowercase();

    // Slide / presentation signals
    let slide_signals = ["slide", "presentasi", "pptx", "powerpoint", "slideshow"];
    let slide_score = count_signals(&lower, &slide_signals);

    // RPP signals
    let rpp_signals = ["rpp", "lesson plan", "rencana pelaksanaan", "rencana pembelajaran"];
    let rpp_score = count_signals(&lower, &rpp_signals);

    // Lembar kerja signals
    let worksheet_signals = ["lembar kerja", "worksheet", "latihan", "soal", "ujian", "tes", "assessment", "penilaian"];
    let worksheet_score = count_signals(&lower, &worksheet_signals);

    // Silabus signals
    let syllabus_signals = ["silabus", "syllabus", "kurikulum"];
    let syllabus_score = count_signals(&lower, &syllabus_signals);

    // Materi pembelajaran signals (default)
    let materi_signals = ["materi", "modul", "belajar", "pembelajaran", "handout", "bahan ajar"];
    let materi_score = count_signals(&lower, &materi_signals);

    // Find the best match
    let scores = [
        (ContentType::SlidePresentasi, slide_score),
        (ContentType::Rpp, rpp_score),
        (ContentType::LembarKerja, worksheet_score),
        (ContentType::Silabus, syllabus_score),
        (ContentType::MateriPembelajaran, materi_score),
    ];

    let max_score = scores.iter().map(|(_, s)| s).copied().fold(0.0_f64, f64::max);

    if max_score < 0.15 {
        return (ContentType::Unknown, 0.0);
    }

    let best = scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    (best.0.clone(), best.1)
}

/// Normalize output type from raw prompt keyword detection.
pub fn detect_output_type(prompt: &str) -> Option<String> {
    let lower = prompt.to_lowercase();

    if lower.contains("pptx") || lower.contains("powerpoint") || lower.contains("slide") && !lower.contains("handout") {
        return Some("pptx".to_string());
    }
    if lower.contains("pdf") || lower.contains("cetak") || lower.contains("print") {
        return Some("pdf".to_string());
    }
    if lower.contains("word") || lower.contains("docx") || lower.contains("edit") {
        return Some("docx".to_string());
    }

    None
}

/// Detect target audience from regex-like pattern matching.
pub fn detect_target_audience(prompt: &str) -> Option<String> {
    let lower = prompt.to_lowercase();

    // Pattern: "kelas X" or "grade X"
    let grade_patterns = [
        ("kelas 1", "SD Kelas 1"), ("kelas 2", "SD Kelas 2"), ("kelas 3", "SD Kelas 3"),
        ("kelas 4", "SD Kelas 4"), ("kelas 5", "SD Kelas 5"), ("kelas 6", "SD Kelas 6"),
        ("kelas 7", "SMP Kelas 7"), ("kelas 8", "SMP Kelas 8"), ("kelas 9", "SMP Kelas 9"),
        ("kelas 10", "SMA Kelas 10"), ("kelas 11", "SMA Kelas 11"), ("kelas 12", "SMA Kelas 12"),
        ("sd", "SD"), ("smp", "SMP"), ("sma", "SMA"),
    ];

    for (pattern, label) in &grade_patterns {
        if lower.contains(pattern) {
            return Some(label.to_string());
        }
    }

    None
}

// ─── Per-type standards ──────────────────────────────────────────────────

fn materi_pembelajaran_standards() -> Vec<FieldDefinition> {
    vec![
        FieldDefinition {
            field_id: "target_audience",
            label_id: "Jenjang/Kelas",
            label_en: "Grade Level",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: grade_level_suggestions(),
        },
        FieldDefinition {
            field_id: "output_type",
            label_id: "Format File",
            label_en: "File Format",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: output_type_suggestions(),
        },
        FieldDefinition {
            field_id: "learning_objectives",
            label_id: "Tujuan Pembelajaran",
            label_en: "Learning Objectives",
            input_type: InputType::TextInput,
            priority: FieldPriority::Recommended,
            suggestions: vec![],
        },
        FieldDefinition {
            field_id: "page_count",
            label_id: "Jumlah Halaman",
            label_en: "Page Count",
            input_type: InputType::Select,
            priority: FieldPriority::Recommended,
            suggestions: page_count_suggestions(),
        },
        FieldDefinition {
            field_id: "include_activities",
            label_id: "Sertakan Latihan?",
            label_en: "Include Exercises?",
            input_type: InputType::Select,
            priority: FieldPriority::Recommended,
            suggestions: vec![
                SuggestionChip { value: "yes".to_string(), label: "Ya, sertakan latihan".to_string() },
                SuggestionChip { value: "no".to_string(), label: "Tidak, materi saja".to_string() },
            ],
        },
    ]
}

fn slide_presentasi_standards() -> Vec<FieldDefinition> {
    vec![
        FieldDefinition {
            field_id: "target_audience",
            label_id: "Jenjang/Kelas",
            label_en: "Grade Level",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: grade_level_suggestions(),
        },
        FieldDefinition {
            field_id: "output_type",
            label_id: "Format File",
            label_en: "File Format",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: vec![
                SuggestionChip { value: "pptx".to_string(), label: "PowerPoint (Presentasi)".to_string() },
            ],
        },
        FieldDefinition {
            field_id: "slide_count",
            label_id: "Jumlah Slide",
            label_en: "Slide Count",
            input_type: InputType::Select,
            priority: FieldPriority::Recommended,
            suggestions: slide_count_suggestions(),
        },
        FieldDefinition {
            field_id: "visual_density",
            label_id: "Tampilan Slide",
            label_en: "Slide Style",
            input_type: InputType::Select,
            priority: FieldPriority::Recommended,
            suggestions: visual_density_suggestions(),
        },
        FieldDefinition {
            field_id: "speaker_notes",
            label_id: "Catatan Presenter",
            label_en: "Speaker Notes",
            input_type: InputType::Select,
            priority: FieldPriority::Optional,
            suggestions: vec![
                SuggestionChip { value: "yes".to_string(), label: "Ya, sertakan catatan".to_string() },
                SuggestionChip { value: "no".to_string(), label: "Tidak".to_string() },
            ],
        },
    ]
}

fn rpp_standards() -> Vec<FieldDefinition> {
    vec![
        FieldDefinition {
            field_id: "target_audience",
            label_id: "Jenjang/Kelas",
            label_en: "Grade Level",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: grade_level_suggestions(),
        },
        FieldDefinition {
            field_id: "learning_objectives",
            label_id: "Tujuan Pembelajaran",
            label_en: "Learning Objectives",
            input_type: InputType::TextInput,
            priority: FieldPriority::Required,
            suggestions: vec![],
        },
        FieldDefinition {
            field_id: "meeting_duration",
            label_id: "Durasi Pertemuan",
            label_en: "Meeting Duration",
            input_type: InputType::Select,
            priority: FieldPriority::Recommended,
            suggestions: meeting_duration_suggestions(),
        },
        FieldDefinition {
            field_id: "teaching_method",
            label_id: "Metode Pembelajaran",
            label_en: "Teaching Method",
            input_type: InputType::MultiSelect,
            priority: FieldPriority::Recommended,
            suggestions: teaching_method_suggestions(),
        },
        FieldDefinition {
            field_id: "assessment_method",
            label_id: "Cara Penilaian",
            label_en: "Assessment Method",
            input_type: InputType::MultiSelect,
            priority: FieldPriority::Recommended,
            suggestions: assessment_method_suggestions(),
        },
    ]
}

fn lembar_kerja_standards() -> Vec<FieldDefinition> {
    vec![
        FieldDefinition {
            field_id: "target_audience",
            label_id: "Jenjang/Kelas",
            label_en: "Grade Level",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: grade_level_suggestions(),
        },
        FieldDefinition {
            field_id: "difficulty_level",
            label_id: "Tingkat Kesulitan",
            label_en: "Difficulty Level",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: difficulty_level_suggestions(),
        },
        FieldDefinition {
            field_id: "page_count",
            label_id: "Jumlah Halaman",
            label_en: "Page Count",
            input_type: InputType::Select,
            priority: FieldPriority::Recommended,
            suggestions: page_count_suggestions(),
        },
        FieldDefinition {
            field_id: "question_count",
            label_id: "Jumlah Soal",
            label_en: "Question Count",
            input_type: InputType::NumberInput,
            priority: FieldPriority::Recommended,
            suggestions: vec![],
        },
        FieldDefinition {
            field_id: "output_type",
            label_id: "Format File",
            label_en: "File Format",
            input_type: InputType::Select,
            priority: FieldPriority::Optional,
            suggestions: output_type_suggestions(),
        },
    ]
}

fn silabus_standards() -> Vec<FieldDefinition> {
    vec![
        FieldDefinition {
            field_id: "target_audience",
            label_id: "Jenjang/Kelas",
            label_en: "Grade Level",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: grade_level_suggestions(),
        },
        FieldDefinition {
            field_id: "learning_objectives",
            label_id: "Tujuan Pembelajaran",
            label_en: "Learning Objectives",
            input_type: InputType::TextInput,
            priority: FieldPriority::Recommended,
            suggestions: vec![],
        },
        FieldDefinition {
            field_id: "output_type",
            label_id: "Format File",
            label_en: "File Format",
            input_type: InputType::Select,
            priority: FieldPriority::Optional,
            suggestions: output_type_suggestions(),
        },
    ]
}

fn penilaian_standards() -> Vec<FieldDefinition> {
    vec![
        FieldDefinition {
            field_id: "target_audience",
            label_id: "Jenjang/Kelas",
            label_en: "Grade Level",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: grade_level_suggestions(),
        },
        FieldDefinition {
            field_id: "difficulty_level",
            label_id: "Tingkat Kesulitan",
            label_en: "Difficulty Level",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: difficulty_level_suggestions(),
        },
        FieldDefinition {
            field_id: "question_count",
            label_id: "Jumlah Soal",
            label_en: "Question Count",
            input_type: InputType::NumberInput,
            priority: FieldPriority::Required,
            suggestions: vec![],
        },
        FieldDefinition {
            field_id: "question_type",
            label_id: "Jenis Soal",
            label_en: "Question Type",
            input_type: InputType::MultiSelect,
            priority: FieldPriority::Recommended,
            suggestions: vec![
                SuggestionChip { value: "pilihan_ganda".to_string(), label: "Pilihan Ganda".to_string() },
                SuggestionChip { value: "essay".to_string(), label: "Essay".to_string() },
                SuggestionChip { value: "uraian".to_string(), label: "Uraian".to_string() },
                SuggestionChip { value: "benar_salah".to_string(), label: "Benar/Salah".to_string() },
                SuggestionChip { value: "isian_singkat".to_string(), label: "Isian Singkat".to_string() },
            ],
        },
        FieldDefinition {
            field_id: "output_type",
            label_id: "Format File",
            label_en: "File Format",
            input_type: InputType::Select,
            priority: FieldPriority::Optional,
            suggestions: output_type_suggestions(),
        },
    ]
}

fn default_standards() -> Vec<FieldDefinition> {
    materi_pembelajaran_standards()
}

// ─── Suggestion chip builders ────────────────────────────────────────────

fn grade_level_suggestions() -> Vec<SuggestionChip> {
    vec![
        SuggestionChip { value: "SD_Kelas_1".to_string(), label: "SD Kelas 1".to_string() },
        SuggestionChip { value: "SD_Kelas_2".to_string(), label: "SD Kelas 2".to_string() },
        SuggestionChip { value: "SD_Kelas_3".to_string(), label: "SD Kelas 3".to_string() },
        SuggestionChip { value: "SD_Kelas_4".to_string(), label: "SD Kelas 4".to_string() },
        SuggestionChip { value: "SD_Kelas_5".to_string(), label: "SD Kelas 5".to_string() },
        SuggestionChip { value: "SD_Kelas_6".to_string(), label: "SD Kelas 6".to_string() },
        SuggestionChip { value: "SMP_Kelas_7".to_string(), label: "SMP Kelas 7".to_string() },
        SuggestionChip { value: "SMP_Kelas_8".to_string(), label: "SMP Kelas 8".to_string() },
        SuggestionChip { value: "SMP_Kelas_9".to_string(), label: "SMP Kelas 9".to_string() },
        SuggestionChip { value: "SMA_Kelas_10".to_string(), label: "SMA Kelas 10".to_string() },
        SuggestionChip { value: "SMA_Kelas_11".to_string(), label: "SMA Kelas 11".to_string() },
        SuggestionChip { value: "SMA_Kelas_12".to_string(), label: "SMA Kelas 12".to_string() },
    ]
}

fn output_type_suggestions() -> Vec<SuggestionChip> {
    vec![
        SuggestionChip { value: "pdf".to_string(), label: "PDF (Untuk Dicetak)".to_string() },
        SuggestionChip { value: "docx".to_string(), label: "Word (Bisa Diedit)".to_string() },
        SuggestionChip { value: "pptx".to_string(), label: "PowerPoint (Presentasi)".to_string() },
    ]
}

fn page_count_suggestions() -> Vec<SuggestionChip> {
    vec![
        SuggestionChip { value: "short".to_string(), label: "Singkat (2-3 halaman)".to_string() },
        SuggestionChip { value: "medium".to_string(), label: "Sedang (5-7 halaman)".to_string() },
        SuggestionChip { value: "long".to_string(), label: "Lengkap (10+ halaman)".to_string() },
    ]
}

fn slide_count_suggestions() -> Vec<SuggestionChip> {
    vec![
        SuggestionChip { value: "short".to_string(), label: "Singkat (8-10 slide)".to_string() },
        SuggestionChip { value: "medium".to_string(), label: "Sedang (15-20 slide)".to_string() },
        SuggestionChip { value: "long".to_string(), label: "Lengkap (25+ slide)".to_string() },
    ]
}

fn meeting_duration_suggestions() -> Vec<SuggestionChip> {
    vec![
        SuggestionChip { value: "35".to_string(), label: "35 Menit".to_string() },
        SuggestionChip { value: "40".to_string(), label: "40 Menit".to_string() },
        SuggestionChip { value: "45".to_string(), label: "45 Menit".to_string() },
        SuggestionChip { value: "2x45".to_string(), label: "2 x 45 Menit (1 JP)".to_string() },
    ]
}

fn teaching_method_suggestions() -> Vec<SuggestionChip> {
    vec![
        SuggestionChip { value: "ceramah".to_string(), label: "Ceramah".to_string() },
        SuggestionChip { value: "diskusi".to_string(), label: "Diskusi".to_string() },
        SuggestionChip { value: "praktik".to_string(), label: "Praktik".to_string() },
        SuggestionChip { value: "inquiry".to_string(), label: "Inkuiri".to_string() },
        SuggestionChip { value: "problem_based".to_string(), label: "Problem Based Learning".to_string() },
        SuggestionChip { value: "project_based".to_string(), label: "Project Based Learning".to_string() },
    ]
}

fn assessment_method_suggestions() -> Vec<SuggestionChip> {
    vec![
        SuggestionChip { value: "written_test".to_string(), label: "Tes Tertulis".to_string() },
        SuggestionChip { value: "oral".to_string(), label: "Tes Lisan".to_string() },
        SuggestionChip { value: "practical".to_string(), label: "Penilaian Praktik".to_string() },
        SuggestionChip { value: "portfolio".to_string(), label: "Portofolio".to_string() },
        SuggestionChip { value: "observation".to_string(), label: "Observasi".to_string() },
    ]
}

fn difficulty_level_suggestions() -> Vec<SuggestionChip> {
    vec![
        SuggestionChip { value: "mudah".to_string(), label: "Mudah".to_string() },
        SuggestionChip { value: "sedang".to_string(), label: "Sedang".to_string() },
        SuggestionChip { value: "sulit".to_string(), label: "Sulit".to_string() },
        SuggestionChip { value: "campuran".to_string(), label: "Campuran".to_string() },
    ]
}

fn visual_density_suggestions() -> Vec<SuggestionChip> {
    vec![
        SuggestionChip { value: "visual".to_string(), label: "Banyak Gambar/Visual".to_string() },
        SuggestionChip { value: "balanced".to_string(), label: "Seimbang Teks & Visual".to_string() },
        SuggestionChip { value: "text_focused".to_string(), label: "Fokus Teks".to_string() },
    ]
}

// ─── Minimum Requirements JSON Schema ───────────────────────────────────

/// A single field in the minimum requirements schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimumRequirementField {
    pub field_id: String,
    pub field_label: String,
    pub priority: String,
    pub input_type: String,
    pub description: String,
    pub suggestions: Vec<SuggestionChip>,
}

/// Minimum requirements for a content type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimumRequirements {
    pub content_type: String,
    pub content_type_label: String,
    pub required_fields: Vec<MinimumRequirementField>,
    pub recommended_fields: Vec<MinimumRequirementField>,
}

/// Get minimum requirements as JSON for a given content type.
///
/// This is used by the LLM prompt and by Rust-side validation to compare
/// interpreted fields against what's actually needed.
pub fn get_minimum_requirements_json(content_type: &ContentType) -> serde_json::Value {
    let reqs = get_minimum_requirements(content_type);
    serde_json::to_value(&reqs).unwrap_or(serde_json::json!({}))
}

/// Get minimum requirements struct for a given content type.
pub fn get_minimum_requirements(content_type: &ContentType) -> MinimumRequirements {
    match content_type {
        ContentType::MateriPembelajaran => MinimumRequirements {
            content_type: "materi_pembelajaran".to_string(),
            content_type_label: "Materi Pembelajaran".to_string(),
            required_fields: vec![
                MinimumRequirementField {
                    field_id: "target_audience".to_string(),
                    field_label: "Jenjang/Kelas".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Jenjang dan kelas siswa yang dituju (misal: SD Kelas 5, SMP Kelas 7)".to_string(),
                    suggestions: grade_level_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "output_type".to_string(),
                    field_label: "Format File".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Format file output: PDF untuk cetak, DOCX untuk diedit, PPTX untuk presentasi".to_string(),
                    suggestions: output_type_suggestions(),
                },
            ],
            recommended_fields: vec![
                MinimumRequirementField {
                    field_id: "learning_objectives".to_string(),
                    field_label: "Tujuan Pembelajaran".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "text_input".to_string(),
                    description: "Apa yang harus dipahami/dikuasai siswa setelah mempelajari materi ini".to_string(),
                    suggestions: vec![],
                },
                MinimumRequirementField {
                    field_id: "page_count".to_string(),
                    field_label: "Jumlah Halaman".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Panjangnya materi: singkat (2-3 hal), sedang (5-7 hal), atau lengkap (10+ hal)".to_string(),
                    suggestions: page_count_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "include_activities".to_string(),
                    field_label: "Sertakan Latihan?".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Apakah perlu disertakan latihan/soal di akhir materi".to_string(),
                    suggestions: vec![
                        SuggestionChip { value: "yes".to_string(), label: "Ya, sertakan latihan".to_string() },
                        SuggestionChip { value: "no".to_string(), label: "Tidak, materi saja".to_string() },
                    ],
                },
            ],
        },
        ContentType::SlidePresentasi => MinimumRequirements {
            content_type: "slide_presentasi".to_string(),
            content_type_label: "Slide Presentasi".to_string(),
            required_fields: vec![
                MinimumRequirementField {
                    field_id: "target_audience".to_string(),
                    field_label: "Jenjang/Kelas".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Jenjang dan kelas siswa yang dituju".to_string(),
                    suggestions: grade_level_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "output_type".to_string(),
                    field_label: "Format File".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Format output: PPTX untuk presentasi".to_string(),
                    suggestions: vec![
                        SuggestionChip { value: "pptx".to_string(), label: "PowerPoint (Presentasi)".to_string() },
                    ],
                },
            ],
            recommended_fields: vec![
                MinimumRequirementField {
                    field_id: "slide_count".to_string(),
                    field_label: "Jumlah Slide".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Jumlah slide: singkat (8-10), sedang (15-20), atau lengkap (25+)".to_string(),
                    suggestions: slide_count_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "visual_density".to_string(),
                    field_label: "Tampilan Slide".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Gaya tampilan: banyak visual, seimbang, atau fokus teks".to_string(),
                    suggestions: visual_density_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "speaker_notes".to_string(),
                    field_label: "Catatan Presenter".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Apakah perlu disertakan catatan untuk presenter".to_string(),
                    suggestions: vec![
                        SuggestionChip { value: "yes".to_string(), label: "Ya, sertakan catatan".to_string() },
                        SuggestionChip { value: "no".to_string(), label: "Tidak".to_string() },
                    ],
                },
            ],
        },
        ContentType::Rpp => MinimumRequirements {
            content_type: "rpp".to_string(),
            content_type_label: "RPP (Rencana Pelaksanaan Pembelajaran)".to_string(),
            required_fields: vec![
                MinimumRequirementField {
                    field_id: "target_audience".to_string(),
                    field_label: "Jenjang/Kelas".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Jenjang dan kelas siswa yang dituju".to_string(),
                    suggestions: grade_level_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "learning_objectives".to_string(),
                    field_label: "Tujuan Pembelajaran".to_string(),
                    priority: "required".to_string(),
                    input_type: "text_input".to_string(),
                    description: "Tujuan pembelajaran yang ingin dicapai".to_string(),
                    suggestions: vec![],
                },
            ],
            recommended_fields: vec![
                MinimumRequirementField {
                    field_id: "meeting_duration".to_string(),
                    field_label: "Durasi Pertemuan".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Lama durasi pertemuan pembelajaran".to_string(),
                    suggestions: meeting_duration_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "teaching_method".to_string(),
                    field_label: "Metode Pembelajaran".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "multi_select".to_string(),
                    description: "Metode yang digunakan: ceramah, diskusi, praktik, dll".to_string(),
                    suggestions: teaching_method_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "assessment_method".to_string(),
                    field_label: "Cara Penilaian".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "multi_select".to_string(),
                    description: "Cara menilai pemahaman siswa".to_string(),
                    suggestions: assessment_method_suggestions(),
                },
            ],
        },
        ContentType::LembarKerja => MinimumRequirements {
            content_type: "lembar_kerja".to_string(),
            content_type_label: "Lembar Kerja".to_string(),
            required_fields: vec![
                MinimumRequirementField {
                    field_id: "target_audience".to_string(),
                    field_label: "Jenjang/Kelas".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Jenjang dan kelas siswa yang dituju".to_string(),
                    suggestions: grade_level_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "difficulty_level".to_string(),
                    field_label: "Tingkat Kesulitan".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Tingkat kesulitan soal: mudah, sedang, sulit, atau campuran".to_string(),
                    suggestions: difficulty_level_suggestions(),
                },
            ],
            recommended_fields: vec![
                MinimumRequirementField {
                    field_id: "page_count".to_string(),
                    field_label: "Jumlah Halaman".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Panjangnya lembar kerja".to_string(),
                    suggestions: page_count_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "question_count".to_string(),
                    field_label: "Jumlah Soal".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "number_input".to_string(),
                    description: "Jumlah soal yang diinginkan".to_string(),
                    suggestions: vec![],
                },
                MinimumRequirementField {
                    field_id: "output_type".to_string(),
                    field_label: "Format File".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Format file output".to_string(),
                    suggestions: output_type_suggestions(),
                },
            ],
        },
        ContentType::Silabus => MinimumRequirements {
            content_type: "silabus".to_string(),
            content_type_label: "Silabus".to_string(),
            required_fields: vec![
                MinimumRequirementField {
                    field_id: "target_audience".to_string(),
                    field_label: "Jenjang/Kelas".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Jenjang dan kelas siswa yang dituju".to_string(),
                    suggestions: grade_level_suggestions(),
                },
            ],
            recommended_fields: vec![
                MinimumRequirementField {
                    field_id: "learning_objectives".to_string(),
                    field_label: "Tujuan Pembelajaran".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "text_input".to_string(),
                    description: "Tujuan pembelajaran yang ingin dicapai".to_string(),
                    suggestions: vec![],
                },
                MinimumRequirementField {
                    field_id: "output_type".to_string(),
                    field_label: "Format File".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Format file output".to_string(),
                    suggestions: output_type_suggestions(),
                },
            ],
        },
        ContentType::Penilaian => MinimumRequirements {
            content_type: "penilaian".to_string(),
            content_type_label: "Penilaian".to_string(),
            required_fields: vec![
                MinimumRequirementField {
                    field_id: "target_audience".to_string(),
                    field_label: "Jenjang/Kelas".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Jenjang dan kelas siswa yang dituju".to_string(),
                    suggestions: grade_level_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "difficulty_level".to_string(),
                    field_label: "Tingkat Kesulitan".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Tingkat kesulitan soal".to_string(),
                    suggestions: difficulty_level_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "question_count".to_string(),
                    field_label: "Jumlah Soal".to_string(),
                    priority: "required".to_string(),
                    input_type: "number_input".to_string(),
                    description: "Jumlah soal yang diinginkan".to_string(),
                    suggestions: vec![],
                },
            ],
            recommended_fields: vec![
                MinimumRequirementField {
                    field_id: "question_type".to_string(),
                    field_label: "Jenis Soal".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "multi_select".to_string(),
                    description: "Jenis soal: pilihan ganda, essay, uraian, dll".to_string(),
                    suggestions: vec![
                        SuggestionChip { value: "pilihan_ganda".to_string(), label: "Pilihan Ganda".to_string() },
                        SuggestionChip { value: "essay".to_string(), label: "Essay".to_string() },
                        SuggestionChip { value: "uraian".to_string(), label: "Uraian".to_string() },
                        SuggestionChip { value: "benar_salah".to_string(), label: "Benar/Salah".to_string() },
                        SuggestionChip { value: "isian_singkat".to_string(), label: "Isian Singkat".to_string() },
                    ],
                },
                MinimumRequirementField {
                    field_id: "output_type".to_string(),
                    field_label: "Format File".to_string(),
                    priority: "recommended".to_string(),
                    input_type: "select".to_string(),
                    description: "Format file output".to_string(),
                    suggestions: output_type_suggestions(),
                },
            ],
        },
        ContentType::Unknown => MinimumRequirements {
            content_type: "unknown".to_string(),
            content_type_label: "Tidak Diketahui".to_string(),
            required_fields: vec![
                MinimumRequirementField {
                    field_id: "target_audience".to_string(),
                    field_label: "Jenjang/Kelas".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Jenjang dan kelas siswa yang dituju".to_string(),
                    suggestions: grade_level_suggestions(),
                },
                MinimumRequirementField {
                    field_id: "output_type".to_string(),
                    field_label: "Format File".to_string(),
                    priority: "required".to_string(),
                    input_type: "select".to_string(),
                    description: "Format file output".to_string(),
                    suggestions: output_type_suggestions(),
                },
            ],
            recommended_fields: vec![],
        },
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────

fn count_signals(text: &str, signals: &[&str]) -> f64 {
    let total = signals.len() as f64;
    let matched = signals.iter().filter(|s| text.contains(*s)).count() as f64;
    if total == 0.0 {
        return 0.0;
    }
    matched / total
}

use serde::{Deserialize, Serialize};

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_content_type_slide() {
        let (ct, score) = detect_content_type("Buatkan slide presentasi tentang pecahan");
        assert_eq!(ct, ContentType::SlidePresentasi);
        assert!(score > 0.3);
    }

    #[test]
    fn test_detect_content_type_rpp() {
        let (ct, _) = detect_content_type("Buatkan RPP untuk kelas 5 SD");
        assert_eq!(ct, ContentType::Rpp);
    }

    #[test]
    fn test_detect_content_type_worksheet() {
        let (ct, _) = detect_content_type("Buatkan lembar kerja latihan soal");
        assert_eq!(ct, ContentType::LembarKerja);
    }

    #[test]
    fn test_detect_content_type_syllabus() {
        let (ct, _) = detect_content_type("Buatkan silabus kurikulum merdeka");
        assert_eq!(ct, ContentType::Silabus);
    }

    #[test]
    fn test_detect_content_type_materi() {
        let (ct, _) = detect_content_type("Buatkan materi pembelajaran tentang gaya");
        assert_eq!(ct, ContentType::MateriPembelajaran);
    }

    #[test]
    fn test_detect_content_type_unknown() {
        let (ct, score) = detect_content_type("hello world");
        assert_eq!(ct, ContentType::Unknown);
        assert!(score < 0.15);
    }

    #[test]
    fn test_detect_output_type_pptx() {
        assert_eq!(detect_output_type("Buatkan slide presentasi"), Some("pptx".to_string()));
    }

    #[test]
    fn test_detect_output_type_pdf() {
        assert_eq!(detect_output_type("Format PDF untuk dicetak"), Some("pdf".to_string()));
    }

    #[test]
    fn test_detect_output_type_docx() {
        assert_eq!(detect_output_type("Buatkan Word document"), Some("docx".to_string()));
    }

    #[test]
    fn test_detect_output_type_none() {
        assert!(detect_output_type("Buatkan materi").is_none());
    }

    #[test]
    fn test_detect_target_audience() {
        assert_eq!(
            detect_target_audience("Buatkan materi untuk kelas 5 SD"),
            Some("SD Kelas 5".to_string())
        );
        assert_eq!(
            detect_target_audience("Untuk kelas 7 SMP"),
            Some("SMP Kelas 7".to_string())
        );
        assert_eq!(
            detect_target_audience("Siswa SMA"),
            Some("SMA".to_string())
        );
    }

    #[test]
    fn test_detect_target_audience_none() {
        assert!(detect_target_audience("Buatkan materi").is_none());
    }

    #[test]
    fn test_get_standards_for_materi() {
        let standards = get_standards_for_content_type(&ContentType::MateriPembelajaran);
        assert!(!standards.is_empty());
        assert_eq!(standards[0].field_id, "target_audience");
        assert_eq!(standards[0].priority, FieldPriority::Required);
    }

    #[test]
    fn test_get_standards_for_slide() {
        let standards = get_standards_for_content_type(&ContentType::SlidePresentasi);
        assert!(!standards.is_empty());
        assert_eq!(standards[0].field_id, "target_audience");
    }

    #[test]
    fn test_get_clarification_fields_max_5() {
        let fields = get_clarification_fields(&ContentType::MateriPembelajaran);
        assert!(fields.len() <= 5);
    }

    #[test]
    fn test_get_clarification_fields_required_first() {
        let fields = get_clarification_fields(&ContentType::MateriPembelajaran);
        for field in &fields {
            assert_ne!(field.priority, FieldPriority::Optional);
        }
    }

    #[test]
    fn test_field_priority_as_str() {
        assert_eq!(FieldPriority::Required.as_str(), "required");
        assert_eq!(FieldPriority::Recommended.as_str(), "recommended");
        assert_eq!(FieldPriority::Optional.as_str(), "optional");
    }

    #[test]
    fn test_input_type_as_str() {
        assert_eq!(InputType::Select.as_str(), "select");
        assert_eq!(InputType::MultiSelect.as_str(), "multi_select");
        assert_eq!(InputType::TextInput.as_str(), "text_input");
        assert_eq!(InputType::NumberInput.as_str(), "number_input");
    }

    #[test]
    fn test_content_type_as_str() {
        assert_eq!(ContentType::MateriPembelajaran.as_str(), "materi_pembelajaran");
        assert_eq!(ContentType::SlidePresentasi.as_str(), "slide_presentasi");
        assert_eq!(ContentType::Rpp.as_str(), "rpp");
        assert_eq!(ContentType::LembarKerja.as_str(), "lembar_kerja");
        assert_eq!(ContentType::Silabus.as_str(), "silabus");
        assert_eq!(ContentType::Penilaian.as_str(), "penilaian");
        assert_eq!(ContentType::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_grade_level_suggestions_count() {
        let suggestions = grade_level_suggestions();
        assert_eq!(suggestions.len(), 12); // SD 1-6, SMP 7-9, SMA 10-12
    }

    #[test]
    fn test_output_type_suggestions_count() {
        let suggestions = output_type_suggestions();
        assert_eq!(suggestions.len(), 3); // pdf, docx, pptx
    }

    #[test]
    fn test_count_signals() {
        assert!(count_signals("hello world", &["hello", "foo"]) > 0.0);
        assert_eq!(count_signals("hello world", &["foo", "bar"]), 0.0);
    }

    // ── Minimum Requirements tests ──────────────────────────────────────

    #[test]
    fn test_get_minimum_requirements_materi() {
        let reqs = get_minimum_requirements(&ContentType::MateriPembelajaran);
        assert_eq!(reqs.content_type, "materi_pembelajaran");
        assert_eq!(reqs.required_fields.len(), 2);
        assert_eq!(reqs.required_fields[0].field_id, "target_audience");
        assert_eq!(reqs.required_fields[1].field_id, "output_type");
        assert_eq!(reqs.recommended_fields.len(), 3);
    }

    #[test]
    fn test_get_minimum_requirements_slide() {
        let reqs = get_minimum_requirements(&ContentType::SlidePresentasi);
        assert_eq!(reqs.content_type, "slide_presentasi");
        assert_eq!(reqs.required_fields.len(), 2);
        assert_eq!(reqs.recommended_fields.len(), 3);
    }

    #[test]
    fn test_get_minimum_requirements_rpp() {
        let reqs = get_minimum_requirements(&ContentType::Rpp);
        assert_eq!(reqs.content_type, "rpp");
        assert_eq!(reqs.required_fields.len(), 2);
        assert_eq!(reqs.required_fields[0].field_id, "target_audience");
        assert_eq!(reqs.required_fields[1].field_id, "learning_objectives");
        assert_eq!(reqs.recommended_fields.len(), 3);
    }

    #[test]
    fn test_get_minimum_requirements_penilaian() {
        let reqs = get_minimum_requirements(&ContentType::Penilaian);
        assert_eq!(reqs.content_type, "penilaian");
        assert_eq!(reqs.required_fields.len(), 3);
        assert_eq!(reqs.recommended_fields.len(), 2);
    }

    #[test]
    fn test_get_minimum_requirements_json_is_valid() {
        let json = get_minimum_requirements_json(&ContentType::MateriPembelajaran);
        assert!(json.is_object());
        assert_eq!(json["content_type"], "materi_pembelajaran");
        assert!(json["required_fields"].is_array());
        assert!(json["recommended_fields"].is_array());
    }

    #[test]
    fn test_minimum_requirements_serialization() {
        let reqs = get_minimum_requirements(&ContentType::MateriPembelajaran);
        let json = serde_json::to_string(&reqs).unwrap();
        assert!(json.contains("target_audience"));
        assert!(json.contains("output_type"));
        assert!(json.contains("materi_pembelajaran"));
    }

    #[test]
    fn test_grade_level_suggestions_in_requirements() {
        let reqs = get_minimum_requirements(&ContentType::MateriPembelajaran);
        let ta_field = &reqs.required_fields[0];
        assert_eq!(ta_field.field_id, "target_audience");
        assert!(!ta_field.suggestions.is_empty());
        assert_eq!(ta_field.suggestions.len(), 12); // SD 1-6, SMP 7-9, SMA 10-12
    }
}
