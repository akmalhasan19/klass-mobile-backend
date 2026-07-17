import json, sys
sys.path.insert(0, "/app")
from app.models import GenerateJobRequest

body = {
    "generation_id": "test123",
    "job_id": "job456",
    "generation_spec": {
        "schema_version": "media_generation_spec.v1",
        "source_interpretation_schema_version": "media_prompt_understanding.v1",
        "export_format": "pdf",
        "title": "Test",
        "language": "id",
        "summary": "Test summary",
        "learning_objectives": [],
        "sections": [{
            "title": "t",
            "purpose": "p",
            "body_blocks": [{"type": "paragraph", "content": "c"}],
            "emphasis": "short"
        }],
        "layout_hints": {
            "document_mode": "document",
            "visual_density": "medium",
            "section_count": 1,
            "asset_count": 0,
            "assessment_block_count": 0
        },
        "style_hints": {
            "tone": "edu",
            "audience_level": "gen",
            "format_preferences": ["pdf"]
        },
        "page_or_slide_structure": {
            "unit_type": "page",
            "total_units": 1,
            "opening_unit": False,
            "section_units": 1,
            "closing_unit": False
        },
        "content_context": {},
        "assets": [],
        "assessment_or_activity_blocks": [],
        "teacher_delivery_summary": "test",
        "contract_versions": {
            "generator_output_metadata": "media_generator_output_metadata.v1"
        }
    },
    "webhook_url": "http://example.com/webhook"
}

try:
    req = GenerateJobRequest.model_validate(body)
    print("OK:", req.model_dump_json()[:200])
except Exception as e:
    print("ERROR:", e)
