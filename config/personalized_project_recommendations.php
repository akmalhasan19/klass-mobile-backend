<?php

use App\Models\RecommendedProject;

return [
    'lock_version' => 'phase_4_2_assignment_tracking_deduplication',

    'files' => [
        'config' => 'config/personalized_project_recommendations.php',
        'mobile_feed_controller' => 'app/Http/Controllers/Api/HomepageRecommendationController.php',
        'admin_configurator_controller' => 'app/Http/Controllers/Admin/AdminHomepageSectionController.php',
        'admin_configurator_view' => 'resources/views/admin/homepage-sections/index.blade.php',
    ],

    'homepage' => [
        'section_key' => 'project_recommendations',
        'feed_endpoint' => '/api/v1/homepage-recommendations',
        'admin_configurator_path' => '/admin/homepage-sections',
        'admin_sections' => [
            'curated_title' => 'Recommended Projects (Admin Curated)',
            'system_distribution_title' => 'Top Distributed System Recommendations by Sub-Subject',
            'system_distribution_description' => 'Read-only summary of the most widely distributed system-generated recommendation for each sub-subject.',
            'system_distribution_empty_state' => 'No system recommendation has been distributed to more than one user yet.',
        ],
    ],

    'distribution_summary' => [
        'eligible_source_types' => [
            RecommendedProject::SOURCE_SYSTEM_TOPIC,
            RecommendedProject::SOURCE_AI_GENERATED,
        ],
        'minimum_distinct_user_count' => 2,
        'maximum_items_per_sub_subject' => 1,
        'tie_breakers' => [
            'distinct_user_count' => 'desc',
            'latest_distribution_at' => 'desc',
            'source_created_at' => 'desc',
            'source_reference' => 'asc',
        ],
    ],

    'fallbacks' => [
        'authenticated_without_personalization' => [
            'mode' => 'default_global_feed',
            'description' => 'Serve the current safe mixed homepage feed when subject profile or authored-topic signals are still insufficient.',
            'tracks_assignments' => true,
        ],
        'guest' => [
            'mode' => 'default_global_feed',
            'description' => 'Guests remain on the current non-personalized homepage feed until an authenticated context exists.',
            'tracks_assignments' => false,
        ],
    ],

    'topic_guardrails' => [
        'taxonomy_required_for_personalization' => true,
        'missing_sub_subject_fallback' => 'general_feed_only',
        'allow_unresolved_ownership_in_general_feed' => true,
        'unresolved_ownership_fallback' => 'general_feed_only',
    ],
];