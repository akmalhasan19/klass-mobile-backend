<?php

return [
    'enabled' => env('CONTENT_INTEGRITY_ENABLED', true),
    
    'classifier_confidence_threshold' => env('CONTENT_INTEGRITY_THRESHOLD', 0.75),
    
    'rejection_strategy' => env('CONTENT_INTEGRITY_REJECTION_STRATEGY', 'warn'),
    // 'strict'  => Reject specs with integrity_score < threshold (generation fails)
    // 'warn'    => Log warnings but allow spec generation (manual review flag)
    // 'log'     => Monitor violations passively (analytics only)
    
    'meta_patterns' => [
        'procedural_instruction' => [
            'follow these steps',
            'implement this',
            'set up',
            'ensure teacher',
            'ensure student',
            'prepare the',
            'prepare students'
        ],
        'conversational_filler' => [
            'here is your',
            'i have generated',
            'i have created',
            'i have prepared',
            'i\'ve',
            'as an ai',
            'as a language model',
            'as claude',
            'as chatgpt',
            'according to my analysis'
        ],
        'structural_scaffolding' => [
            'this section is designed to',
            'this lesson aims to',
            'this activity will',
            'this focuses on',
            'focus on the following',
            'be sure to',
            'the purpose of this'
        ],
    ],
    
    'kurikulum_merdeka_reference' => resource_path('json/kurikulum_merdeka_structure.json'),
];
