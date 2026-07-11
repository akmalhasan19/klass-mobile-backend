<?php

namespace Tests\Feature;

use App\Services\MediaPromptTaxonomyInferenceService;
use Database\Seeders\SubjectTaxonomySeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class MediaPromptTaxonomyInferenceServiceTest extends TestCase
{
    use RefreshDatabase;

    public function test_prompt_taxonomy_inference_matches_sd_smp_sma_and_smk_examples(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $service = new MediaPromptTaxonomyInferenceService();

        $sdMatch = $service->infer('Buatkan PDF pembelajaran IPAS kelas 4 tentang Gaya di Sekitar Kita lengkap dengan contoh fenomena dan eksperimen aman.');
        $smpMatch = $service->infer('Buatkan modul Pendidikan Pancasila kelas 7 tentang Sejarah Kelahiran Pancasila dengan contoh penerapan dan latihan.');
        $smaMatch = $service->infer('Buatkan handout matematika tingkat lanjut kelas 12 tentang integral dan aplikasi luas daerah.');
        $smkMatch = $service->infer('Buatkan job sheet SMK kelas 10 Teknik Otomotif tentang K3LH dan Budaya Kerja lengkap dengan evaluasi kerja.');

        $this->assertSame('ipas-sd', data_get($sdMatch, 'best_match.subject_slug'));
        $this->assertSame('gaya-sekitar-kita-kelas-4', data_get($sdMatch, 'best_match.sub_subject_slug'));
        $this->assertNotNull(data_get($sdMatch, 'best_match.subject_id'));
        $this->assertNotNull(data_get($sdMatch, 'best_match.sub_subject_id'));

        $this->assertSame('pendidikan-pancasila-smp', data_get($smpMatch, 'best_match.subject_slug'));
        $this->assertSame('sejarah-kelahiran-pancasila-kelas-7', data_get($smpMatch, 'best_match.sub_subject_slug'));

        $this->assertSame('matematika-tingkat-lanjut-sma', data_get($smaMatch, 'best_match.subject_slug'));
        $this->assertSame('kalkulus-lanjut-kelas-12', data_get($smaMatch, 'best_match.sub_subject_slug'));

        $this->assertSame('teknik-otomotif-smk', data_get($smkMatch, 'best_match.subject_slug'));
        $this->assertSame('k3lh-budaya-kerja-kelas-10', data_get($smkMatch, 'best_match.sub_subject_slug'));
        $this->assertNotEmpty(data_get($smkMatch, 'best_match.structure_items', []));
    }

    public function test_prompt_taxonomy_inference_can_keep_subject_without_forcing_ambiguous_sub_subject(): void
    {
        $this->seed(SubjectTaxonomySeeder::class);

        $match = (new MediaPromptTaxonomyInferenceService())->infer(
            'Buatkan modul Pendidikan Pancasila untuk siswa kelas 7 SMP dengan bahasa Indonesia semi-formal.'
        );

        $this->assertSame('pendidikan-pancasila-smp', data_get($match, 'best_match.subject_slug'));
        $this->assertNull(data_get($match, 'best_match.sub_subject_slug'));
        $this->assertFalse((bool) data_get($match, 'confidence.sub_subject_attached'));
    }
}