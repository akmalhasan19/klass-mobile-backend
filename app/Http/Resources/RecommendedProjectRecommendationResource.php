<?php

namespace App\Http\Resources;

use Carbon\CarbonInterface;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;
use Illuminate\Support\Collection;

class RecommendedProjectRecommendationResource extends JsonResource
{
    public function toArray(Request $request): array
    {
        return [
            'id' => (string) data_get($this->resource, 'id'),
            'title' => data_get($this->resource, 'title'),
            'description' => data_get($this->resource, 'description'),
            'thumbnail_url' => data_get($this->resource, 'thumbnail_url'),
            'ratio' => data_get($this->resource, 'ratio', '16:9'),
            'project_type' => data_get($this->resource, 'project_type'),
            'tags' => $this->normalizeList(data_get($this->resource, 'tags')),
            'modules' => $this->normalizeList(data_get($this->resource, 'modules')),
            'sub_subject_id' => data_get($this->resource, 'sub_subject_id'),
            'subject_id' => data_get($this->resource, 'subject_id'),
            'taxonomy' => data_get($this->resource, 'taxonomy'),
            'personalization' => $this->when(
                data_get($this->resource, 'personalization') !== null,
                data_get($this->resource, 'personalization')
            ),
            'source_type' => data_get($this->resource, 'source_type'),
            'display_priority' => (int) data_get($this->resource, 'display_priority', 0),
            'visibility' => [
                'is_active' => (bool) data_get($this->resource, 'visibility.is_active', false),
                'starts_at' => $this->serializeTimestamp(data_get($this->resource, 'visibility.starts_at')),
                'ends_at' => $this->serializeTimestamp(data_get($this->resource, 'visibility.ends_at')),
            ],
            'source_reference' => $this->when(
                $request->boolean('include_source_context'),
                data_get($this->resource, 'source_reference')
            ),
            'source_payload' => $this->when(
                $request->boolean('include_source_context'),
                data_get($this->resource, 'source_payload', [])
            ),
            'created_at' => $this->serializeTimestamp(data_get($this->resource, 'created_at')),
            'updated_at' => $this->serializeTimestamp(data_get($this->resource, 'updated_at')),
        ];
    }

    /**
     * @param  mixed  $value
     * @return array<int, mixed>
     */
    protected function normalizeList(mixed $value): array
    {
        if ($value instanceof Collection) {
            return $value->values()->all();
        }

        if (is_array($value)) {
            return array_values($value);
        }

        return [];
    }

    protected function serializeTimestamp(mixed $value): ?string
    {
        if ($value instanceof CarbonInterface) {
            return $value->toISOString();
        }

        return is_string($value) ? $value : null;
    }
}