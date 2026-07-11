<?php

namespace App\Http\Resources;

use App\Models\RecommendedProject;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\ResourceCollection;

class RecommendedProjectRecommendationCollection extends ResourceCollection
{
    public $collects = RecommendedProjectRecommendationResource::class;

    /**
     * @var array<string, mixed>
     */
    protected array $contextMeta = [];

    public function toArray(Request $request): array
    {
        return parent::toArray($request);
    }

    public function with(Request $request): array
    {
        $items = collect($this->resource);

        return [
            'meta' => array_replace_recursive([
                'total' => $items->count(),
                'source_breakdown' => [
                    RecommendedProject::SOURCE_ADMIN_UPLOAD => $items->where('source_type', RecommendedProject::SOURCE_ADMIN_UPLOAD)->count(),
                    RecommendedProject::SOURCE_SYSTEM_TOPIC => $items->where('source_type', RecommendedProject::SOURCE_SYSTEM_TOPIC)->count(),
                    RecommendedProject::SOURCE_AI_GENERATED => $items->where('source_type', RecommendedProject::SOURCE_AI_GENERATED)->count(),
                ],
            ], $this->contextMeta),
        ];
    }

    /**
     * @param  array<string, mixed>  $contextMeta
     */
    public function withContextMeta(array $contextMeta): self
    {
        $this->contextMeta = $contextMeta;

        return $this;
    }
}