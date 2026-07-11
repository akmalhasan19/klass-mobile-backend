<?php

namespace App\MediaGeneration;

use Illuminate\Support\Str;

final class SubjectsJsonTaxonomyCatalog
{
    /**
     * @var array<int, array<string, mixed>>|null
     */
    private static ?array $entries = null;

    /**
     * @return array<int, array<string, mixed>>
     */
    public static function entries(): array
    {
        if (self::$entries !== null) {
            return self::$entries;
        }

        $path = self::path();

        if (! is_file($path)) {
            self::$entries = [];

            return self::$entries;
        }

        $contents = @file_get_contents($path);

        if (! is_string($contents) || trim($contents) === '') {
            self::$entries = [];

            return self::$entries;
        }

        $decoded = json_decode($contents, true);

        if (! is_array($decoded)) {
            self::$entries = [];

            return self::$entries;
        }

        $entries = [];

        foreach ($decoded as $index => $row) {
            $normalized = self::normalizeEntry($row, $index);

            if ($normalized !== null) {
                $entries[] = $normalized;
            }
        }

        self::$entries = $entries;

        return self::$entries;
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    public static function groupedSubjects(): array
    {
        $grouped = [];

        foreach (self::entries() as $entry) {
            $subjectSlug = $entry['subject_slug'];

            if (! isset($grouped[$subjectSlug])) {
                $grouped[$subjectSlug] = [
                    'subject_name' => $entry['subject_name'],
                    'subject_slug' => $subjectSlug,
                    'jenjang' => $entry['jenjang'],
                    'is_active' => $entry['is_active'],
                    'entries' => [],
                ];
            }

            $grouped[$subjectSlug]['is_active'] = (bool) $grouped[$subjectSlug]['is_active'] || (bool) $entry['is_active'];
            $grouped[$subjectSlug]['entries'][] = $entry;
        }

        return array_values($grouped);
    }

    public static function flushCache(): void
    {
        self::$entries = null;
    }

    public static function path(): string
    {
        return dirname(base_path()) . DIRECTORY_SEPARATOR . 'subjects.json';
    }

    public static function normalizeSearchText(string $value): string
    {
        $normalized = Str::ascii($value);
        $normalized = strtolower($normalized);
        $normalized = preg_replace('/[^\p{L}\p{N}]+/u', ' ', $normalized) ?? $normalized;
        $normalized = preg_replace('/\s+/u', ' ', trim($normalized)) ?? trim($normalized);

        return $normalized;
    }

    /**
     * @return string[]
     */
    public static function tokenizeSearchText(string $value): array
    {
        $normalized = self::normalizeSearchText($value);

        if ($normalized === '') {
            return [];
        }

        $tokens = preg_split('/\s+/u', $normalized) ?: [];

        return array_values(array_filter($tokens, static fn (string $token): bool => $token !== ''));
    }

    /**
     * @param  array<string, mixed>|mixed  $row
     * @return array<string, mixed>|null
     */
    private static function normalizeEntry(mixed $row, int $index): ?array
    {
        if (! is_array($row)) {
            return null;
        }

        $subjectName = trim((string) ($row['subject'] ?? ''));
        $subSubjectName = trim((string) ($row['sub_subject'] ?? ''));

        if ($subjectName === '' || $subSubjectName === '') {
            return null;
        }

        $description = self::normalizeTextValue($row['deskripsi_singkat'] ?? $row['description'] ?? '');
        $rawContentStructure = $row['Structure of content'] ?? $row['structure_of_content'] ?? '';
        $contentStructure = self::normalizeTextValue($rawContentStructure);

        return [
            'catalog_index' => $index + 1,
            'jenjang' => self::normalizeJenjang($row['jenjang'] ?? null),
            'subject_name' => $subjectName,
            'subject_slug' => self::normalizeSlug($row['subject_slug'] ?? null, $subjectName),
            'kelas' => self::normalizeInteger($row['kelas'] ?? null),
            'semester' => self::normalizeInteger($row['semester'] ?? null),
            'bab' => self::normalizeInteger($row['bab'] ?? null),
            'sub_subject_name' => $subSubjectName,
            'sub_subject_slug' => self::normalizeSlug($row['sub_subject_slug'] ?? null, $subSubjectName),
            'description' => $description,
            'is_active' => self::normalizeBool($row['is_active'] ?? true),
            'content_structure' => $contentStructure,
            'structure_items' => self::structureItems($rawContentStructure),
            'normalized_subject' => self::normalizeSearchText($subjectName),
            'normalized_sub_subject' => self::normalizeSearchText($subSubjectName),
            'normalized_description' => self::normalizeSearchText($description),
        ];
    }

    private static function normalizeJenjang(mixed $value): ?string
    {
        $normalized = strtoupper(trim((string) $value));

        return $normalized !== '' ? $normalized : null;
    }

    private static function normalizeSlug(mixed $value, string $fallback): string
    {
        $slug = trim((string) $value);

        return Str::slug($slug !== '' ? $slug : $fallback);
    }

    private static function normalizeInteger(mixed $value): ?int
    {
        if ($value === null || $value === '') {
            return null;
        }

        return is_numeric($value) ? (int) $value : null;
    }

    private static function normalizeBool(mixed $value): bool
    {
        $normalized = filter_var($value, FILTER_VALIDATE_BOOL, FILTER_NULL_ON_FAILURE);

        return $normalized ?? (bool) $value;
    }

    /**
     * @return string[]
     */
    private static function normalizeTextValue(mixed $value): string
    {
        if (is_string($value)) {
            return trim($value);
        }

        if (is_array($value)) {
            $parts = array_values(array_filter(array_map(
                static fn (mixed $part): string => self::normalizeTextValue($part),
                $value
            ), static fn (string $part): bool => $part !== ''));

            return implode(', ', $parts);
        }

        if (is_scalar($value)) {
            return trim((string) $value);
        }

        return '';
    }

    private static function structureItems(mixed $contentStructure): array
    {
        if (is_array($contentStructure)) {
            return array_values(array_filter(array_map(
                static fn (mixed $part): string => Str::of(self::normalizeTextValue($part))->trim()->toString(),
                $contentStructure
            ), static fn (string $part): bool => $part !== ''));
        }

        $normalizedContentStructure = self::normalizeTextValue($contentStructure);

        if ($normalizedContentStructure === '') {
            return [];
        }

        $normalized = preg_replace('/\s+(dan|and)\s+/iu', ', ', $normalizedContentStructure) ?? $normalizedContentStructure;
        $parts = preg_split('/\s*,\s*/u', $normalized) ?: [];

        return array_values(array_filter(array_map(
            static function (string $part): string {
                return Str::of($part)
                    ->replaceMatches('/\s+/u', ' ')
                    ->trim()
                    ->toString();
            },
            $parts
        ), static fn (string $part): bool => $part !== ''));
    }
}