<?php

namespace Tests\Feature;

use App\MediaGeneration\MediaGenerationLifecycle;
use App\Models\MediaGeneration;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AdminMediaGenerationDebugPageTest extends TestCase
{
    use RefreshDatabase;

    public function test_admin_can_open_media_generation_debug_page_and_filter_results(): void
    {
        $admin = User::factory()->admin()->create();
        $teacher = User::factory()->teacher()->create([
            'name' => 'Bu Rani',
            'email' => 'rani.teacher@example.test',
        ]);

        $matchingGeneration = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout IPAS kelas 4 tentang gaya di sekitar kita.',
            'preferred_output_type' => 'auto',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
        ]);

        MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan deck kimia kelas 10 tentang tabel periodik.',
            'preferred_output_type' => 'pptx',
            'resolved_output_type' => 'pptx',
            'status' => MediaGenerationLifecycle::FAILED,
        ]);

        $response = $this->actingAs($admin)->get(route('admin.media-generations.index', [
            'status' => MediaGenerationLifecycle::COMPLETED,
            'search' => 'IPAS',
            'generation' => $matchingGeneration->id,
        ]));

        $response
            ->assertOk()
            ->assertSeeText('Media Generation Debug')
            ->assertSeeText('Buatkan handout IPAS kelas 4 tentang gaya di sekitar kita.')
            ->assertDontSeeText('Buatkan deck kimia kelas 10 tentang tabel periodik.')
            ->assertSee(url('/api/v1/admin/media-generations/' . $matchingGeneration->id . '/debug-taxonomy'), false)
            ->assertViewHas('selectedGenerationId', (string) $matchingGeneration->id)
            ->assertViewHas('status', MediaGenerationLifecycle::COMPLETED);

        $this->assertCount(1, $response->viewData('mediaGenerations')->items());
    }

    public function test_admin_web_session_can_access_taxonomy_debug_api_used_by_the_page(): void
    {
        $admin = User::factory()->admin()->create();
        $teacher = User::factory()->teacher()->create();

        $generation = MediaGeneration::create([
            'teacher_id' => $teacher->id,
            'raw_prompt' => 'Buatkan handout IPAS kelas 4 tentang gaya di sekitar kita.',
            'preferred_output_type' => 'auto',
            'resolved_output_type' => 'pdf',
            'status' => MediaGenerationLifecycle::COMPLETED,
            'interpretation_payload' => [
                'subject_context' => [
                    'subject_name' => 'IPAS',
                    'subject_slug' => 'ipas-sd',
                ],
                'sub_subject_context' => [
                    'sub_subject_name' => 'Gaya di Sekitar Kita',
                    'sub_subject_slug' => 'gaya-sekitar-kita-kelas-4',
                ],
            ],
            'interpretation_audit_payload' => [
                'taxonomy_inference' => [
                    'confidence' => [
                        'score' => 0.91,
                        'label' => 'high',
                    ],
                    'best_match' => [
                        'subject_name' => 'IPAS',
                        'subject_slug' => 'ipas-sd',
                        'sub_subject_name' => 'Gaya di Sekitar Kita',
                        'sub_subject_slug' => 'gaya-sekitar-kita-kelas-4',
                        'matched_signals' => ['subject_phrase', 'sub_subject_phrase'],
                        'structure_items' => ['Pengertian gaya', 'Contoh gaya dorong dan tarik'],
                    ],
                    'candidate_matches' => [],
                ],
            ],
            'decision_payload' => [
                'content_draft' => [
                    'source' => 'adapter',
                    'schema_version' => 'media_content_draft.v1',
                    'draft_fallback_triggered' => false,
                    'draft_fallback_reason_code' => null,
                    'taxonomy_hint' => [
                        'schema_version' => 'media_draft_taxonomy_hint.v1',
                        'source' => 'prompt_inference',
                        'confidence' => [
                            'score' => 0.91,
                            'label' => 'high',
                        ],
                        'subject' => [
                            'id' => null,
                            'name' => 'IPAS',
                            'slug' => 'ipas-sd',
                        ],
                        'sub_subject' => [
                            'id' => null,
                            'subject_id' => null,
                            'name' => 'Gaya di Sekitar Kita',
                            'slug' => 'gaya-sekitar-kita-kelas-4',
                        ],
                        'grade_context' => [
                            'jenjang' => 'sd',
                            'kelas' => '4',
                            'semester' => '2',
                            'bab' => '6',
                        ],
                        'content_guidance' => [
                            'description' => 'Membahas gaya dorong dan tarik dalam kehidupan sehari-hari.',
                            'structure' => 'Pengertian, contoh, dan latihan singkat.',
                            'structure_items' => ['Pengertian gaya', 'Contoh gaya dorong dan tarik'],
                        ],
                        'matched_signals' => ['subject_phrase', 'sub_subject_phrase'],
                    ],
                ],
            ],
        ]);

        $this->actingAs($admin)
            ->getJson('/api/v1/admin/media-generations/' . $generation->id . '/debug-taxonomy')
            ->assertOk()
            ->assertJsonPath('data.id', $generation->id)
            ->assertJsonPath('data.taxonomy_inference.best_match.subject_slug', 'ipas-sd')
            ->assertJsonPath('data.draft_taxonomy_hint.source', 'prompt_inference');
    }
}