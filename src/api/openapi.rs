use utoipa::OpenApi;
use utoipa::Modify;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Klass Gateway API",
        version = "1.0.0",
        description = "API Gateway for Klass — AI-powered educational content generation"
    ),
    paths(
        // Auth
        crate::api::rest::auth::register,
        crate::api::rest::auth::login,
        crate::api::rest::auth::logout,
        crate::api::rest::auth::me,
        crate::api::rest::auth::refresh,
        crate::api::rest::auth::get_security_question,
        crate::api::rest::auth::verify_and_reset_password,
        // User
        crate::api::rest::avatar::upload_avatar,
        // Topics (public)
        crate::api::rest::topics::index,
        crate::api::rest::topics::show,
        // Contents (public)
        crate::api::rest::contents::index,
        crate::api::rest::contents::show,
        // Marketplace tasks (public)
        crate::api::rest::marketplace_tasks::index,
        crate::api::rest::marketplace_tasks::show,
        // Media generations (teacher)
        crate::api::rest::media_generations::create,
        crate::api::rest::media_generations::index,
        crate::api::rest::media_generations::show,
        crate::api::rest::media_generations::regenerate,
        crate::api::rest::media_generations::job_status,
        // NOTE: `POST /internal/media-generations/webhook`
        // (crate::api::rest::media_webhook::webhook_handler) is intentionally
        // EXCLUDED from the public OpenAPI docs — it is an internal, HMAC-signed
        // endpoint used only by the Python Arq worker to report job completion.
        // Freelancer
        crate::api::rest::freelancer::suggest_freelancers,
        crate::api::rest::freelancer::hire_freelancer,
        // Student progress (public)
        crate::api::rest::student_progress::index,
        crate::api::rest::student_progress::show,
        // Homepage sections (public)
        crate::api::rest::homepage_sections::index,
        // Gallery
        crate::api::rest::gallery::index,
        // Homepage recommendations
        crate::api::rest::homepage_recommendations::index,
        // Admin activity logs
        crate::api::rest::admin::activity_logs::index,
        // Admin contents
        crate::api::rest::admin::contents::create,
        crate::api::rest::admin::contents::update,
        crate::api::rest::admin::contents::delete,
        crate::api::rest::admin::contents::reorder,
        crate::api::rest::admin::contents::publish,
        // Admin homepage sections
        crate::api::rest::admin::homepage_sections::bulk_update,
        // Admin marketplace tasks
        crate::api::rest::admin::marketplace_tasks::create,
        crate::api::rest::admin::marketplace_tasks::update,
        crate::api::rest::admin::marketplace_tasks::delete,
        crate::api::rest::admin::marketplace_tasks::update_status,
        // Admin media generations
        crate::api::rest::admin::media_generations::index,
        crate::api::rest::admin::media_generations_debug::debug_taxonomy,
        // Admin recommended projects
        crate::api::rest::admin::recommended_projects::create,
        crate::api::rest::admin::recommended_projects::update,
        crate::api::rest::admin::recommended_projects::delete,
        crate::api::rest::admin::recommended_projects::toggle_active,
        crate::api::rest::admin::recommended_projects::show_now,
        // Admin student progress
        crate::api::rest::admin::student_progress::create,
        crate::api::rest::admin::student_progress::update,
        crate::api::rest::admin::student_progress::delete,
        // Admin system settings
        crate::api::rest::admin::system_settings::index,
        crate::api::rest::admin::system_settings::bulk_update,
        // Admin topics
        crate::api::rest::admin::topics::update,
        crate::api::rest::admin::topics::delete,
        crate::api::rest::admin::topics::reorder,
        crate::api::rest::admin::topics::publish,
        // Admin uploads
        crate::api::rest::admin::uploads::upload,
        crate::api::rest::admin::uploads::delete,
    ),
    components(
        schemas(
            // Auth
            crate::api::rest::auth::UserResource,
            crate::api::rest::auth::AuthData,
            crate::api::rest::auth::TokenOnlyData,
            crate::api::rest::auth::SecurityQuestionData,
            crate::api::rest::auth::MessageData,
            crate::api::rest::auth::RegisterRequest,
            crate::api::rest::auth::LoginRequest,
            crate::api::rest::auth::GetSecurityQuestionRequest,
            crate::api::rest::auth::VerifyAndResetPasswordRequest,
            // Avatar
            crate::api::rest::avatar::AvatarData,
            // Topics (public)
            crate::api::rest::topics::SubjectResource,
            crate::api::rest::topics::SubSubjectResource,
            crate::api::rest::topics::TaxonomyResource,
            crate::api::rest::topics::PersonalizationResource,
            crate::api::rest::topics::MarketplaceTaskResource,
            crate::api::rest::topics::ContentResource,
            crate::api::rest::topics::TopicResource,
            crate::api::rest::topics::TopicQueryParams,
            // Contents (public)
            crate::api::rest::contents::MarketplaceTaskResource,
            crate::api::rest::contents::TopicResource,
            crate::api::rest::contents::ContentResource,
            crate::api::rest::contents::ContentQueryParams,
            // Marketplace tasks (public)
            crate::api::rest::marketplace_tasks::MarketplaceTaskResource,
            crate::api::rest::marketplace_tasks::ContentResource,
            crate::api::rest::marketplace_tasks::MarketplaceTaskQueryParams,
            // Student progress (public)
            crate::api::rest::student_progress::StudentProgressResource,
            crate::api::rest::student_progress::StudentProgressQueryParams,
            // Homepage sections (public)
            crate::api::rest::homepage_sections::HomepageSectionResource,
            // Gallery
            crate::api::rest::gallery::ContentResource,
            crate::api::rest::gallery::TopicResource,
            crate::api::rest::gallery::GalleryQueryParams,
            // Homepage recommendations
            crate::api::rest::homepage_recommendations::RecommendedProjectResource,
            crate::api::rest::homepage_recommendations::PersonalizationResource,
            crate::api::rest::homepage_recommendations::VisibilityResource,
            crate::api::rest::homepage_recommendations::SectionMeta,
            crate::api::rest::homepage_recommendations::LimitMeta,
            crate::api::rest::homepage_recommendations::PersonalizationMeta,
            crate::api::rest::homepage_recommendations::SourceStatusResource,
            crate::api::rest::homepage_recommendations::Meta,
            crate::api::rest::homepage_recommendations::Response,
            crate::api::rest::homepage_recommendations::HomepageRecommendationsQuery,
            // Media generations (teacher)
            crate::api::rest::media_generations::SubjectResource,
            crate::api::rest::media_generations::SubSubjectResource,
            crate::api::rest::media_generations::TopicResource,
            crate::api::rest::media_generations::ContentResource,
            crate::api::rest::media_generations::RecommendedProjectResource,
            crate::api::rest::media_generations::MediaGenerationResource,
            crate::api::rest::media_generations::MediaGenerationListResponse,
            crate::api::rest::media_generations::MediaGenerationChainResource,
            crate::api::rest::media_generations::CreateMediaGenerationRequest,
            crate::api::rest::media_generations::MediaGenerationQueryParams,
            crate::api::rest::media_generations::RegenerateRequest,
            // Async job tracking (Task 1.3)
            crate::api::rest::media_generations::CreateMediaGenerationResponse,
            crate::api::rest::media_generations::JobStatusResponse,
            // Freelancer
            crate::api::rest::freelancer::FreelancerMatchResource,
            crate::api::rest::freelancer::HiredFreelancerResource,
            crate::api::rest::freelancer::SuggestFreelancersRequest,
            crate::api::rest::freelancer::HireFreelancerRequest,
            crate::api::rest::freelancer::HireMode,
            // Admin activity logs
            crate::api::rest::admin::activity_logs::ActivityLogResource,
            crate::api::rest::admin::activity_logs::ActivityLogQueryParams,
            // Admin contents
            crate::api::rest::admin::contents::CreateContentRequest,
            crate::api::rest::admin::contents::UpdateContentRequest,
            crate::api::rest::admin::contents::ReorderRequest,
            crate::api::rest::admin::contents::PublishRequest,
            // Admin homepage sections
            crate::api::rest::admin::homepage_sections::BulkUpdateSection,
            crate::api::rest::admin::homepage_sections::BulkUpdateHomepageSectionsRequest,
            // Admin marketplace tasks
            crate::api::rest::admin::marketplace_tasks::CreateMarketplaceTaskRequest,
            crate::api::rest::admin::marketplace_tasks::UpdateMarketplaceTaskRequest,
            crate::api::rest::admin::marketplace_tasks::UpdateStatusRequest,
            // Admin media generations
            crate::api::rest::admin::media_generations::AdminTeacherSummary,
            crate::api::rest::admin::media_generations::AdminMediaGenerationResource,
            crate::api::rest::admin::media_generations::AdminMediaGenerationQueryParams,
            // Admin media generations debug
            crate::api::rest::admin::media_generations_debug::MediaGenerationTaxonomyDebugResource,
            // Admin recommended projects
            crate::api::rest::admin::recommended_projects::RecommendedProjectResource,
            crate::api::rest::admin::recommended_projects::UpdateRecommendedProjectRequest,
            // Admin student progress
            crate::api::rest::admin::student_progress::CreateStudentProgressRequest,
            crate::api::rest::admin::student_progress::UpdateStudentProgressRequest,
            // Admin system settings
            crate::api::rest::admin::system_settings::SettingResource,
            crate::api::rest::admin::system_settings::BulkUpdateSettingsRequest,
            // Admin topics
            crate::api::rest::admin::topics::UpdateTopicRequest,
            crate::api::rest::admin::topics::ReorderRequest,
            crate::api::rest::admin::topics::PublishRequest,
            // Admin uploads
            crate::api::rest::admin::uploads::DeleteUploadQuery,
        )
    ),
    tags(
        (name = "auth", description = "Authentication endpoints (register, login, logout, profile, password reset)"),
        (name = "user", description = "User profile endpoints"),
        (name = "topics", description = "Public read-only topics"),
        (name = "contents", description = "Public read-only contents"),
        (name = "marketplace-tasks", description = "Public read-only marketplace tasks"),
        (name = "student-progress", description = "Public read-only student progress"),
        (name = "homepage-sections", description = "Public homepage sections"),
        (name = "gallery", description = "Public gallery"),
        (name = "homepage-recommendations", description = "Homepage recommendations"),
        (name = "media-generations", description = "Teacher media generation management"),
        (name = "admin-activity-logs", description = "Admin activity logs viewer"),
        (name = "admin-contents", description = "Admin content CRUD"),
        (name = "admin-homepage-sections", description = "Admin homepage section management"),
        (name = "admin-marketplace-tasks", description = "Admin marketplace task management"),
        (name = "admin-media-generations", description = "Admin media generation listing and debug"),
        (name = "admin-recommended-projects", description = "Admin recommended projects CRUD"),
        (name = "admin-student-progress", description = "Admin student progress CRUD"),
        (name = "admin-settings", description = "Admin system settings"),
        (name = "admin-topics", description = "Admin topic management"),
        (name = "admin-uploads", description = "Admin file uploads"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("UUID|hex")
                        .description(Some("Personal access token: `{id}|{hex_token}`"))
                        .build(),
                ),
            );
        }
    }
}
