<p align="center"><a href="https://laravel.com" target="_blank"><img src="https://raw.githubusercontent.com/laravel/art/master/logo-lockup/5%20SVG/2%20CMYK/1%20Full%20Color/laravel-logolockup-cmyk-red.svg" width="400" alt="Laravel Logo"></a></p>

<p align="center">
<a href="https://github.com/laravel/framework/actions"><img src="https://github.com/laravel/framework/workflows/tests/badge.svg" alt="Build Status"></a>
<a href="https://packagist.org/packages/laravel/framework"><img src="https://img.shields.io/packagist/dt/laravel/framework" alt="Total Downloads"></a>
<a href="https://packagist.org/packages/laravel/framework"><img src="https://img.shields.io/packagist/v/laravel/framework" alt="Latest Stable Version"></a>
<a href="https://packagist.org/packages/laravel/framework"><img src="https://img.shields.io/packagist/l/laravel/framework" alt="License"></a>
</p>

## Klass Backend Notes

Phase 8 operational documentation for personalized project recommendations lives in `PERSONALIZED_PROJECT_RECOMMENDATIONS.md`. Use that file for runtime flow, schema, fallback policy, aggregation rules, and deployment/backfill steps.

### Homepage Configurator / Recommended Projects

- Admin workspace lives at `/admin/homepage-sections` and now manages both section ordering and curated recommended projects.
- Recommended project CRUD routes are under `/admin/homepage-sections/recommended-projects/*` and every create, update, toggle, and delete action writes an `activity_logs` row.
- Thumbnail uploads from Homepage Configurator reuse `FileUploadService` with the `gallery` upload category on the `supabase` disk.
- Public mobile feed for mixed recommendations is served by `GET /api/homepage-recommendations` and stays gated by the `homepage_sections` visibility config for `project_recommendations`.
- Curated admin uploads always remain visible in the feed when the section is enabled; only system-generated candidates participate in personalization and distribution tracking.
- Phase 0 through Phase 8 decisions for personalized recommendations now live in `config/personalized_project_recommendations.php` plus `PERSONALIZED_PROJECT_RECOMMENDATIONS.md`, including the admin section terminology, summary eligibility rules, deterministic tie-breakers, guest/authenticated fallback policy, and deployment runbook.

### Topic Taxonomy / Ownership Normalization

- Subject taxonomy now lives in `subjects` and `sub_subjects`, seeded via `SubjectTaxonomySeeder` for future personalization and admin aggregation work.
- Topics now point to taxonomy through `topics.sub_subject_id`; `subject_id` is derived from the chosen sub-subject instead of being duplicated on the topics table.
- Topic ownership is now normalized through `topics.owner_user_id` plus `topics.ownership_status`, while legacy `topics.teacher_id` remains for backward compatibility.
- User profile anchors now live in `users.primary_subject_id`, and the personalization subject should resolve from profile first, then fall back to authored-topic activity when no profile subject is set.
- Existing topic rows are backfilled during migration by mapping numeric `teacher_id` values to `users.id` and email-like `teacher_id` values to `users.email`.
- Legacy topics that still cannot be mapped remain marked as `legacy_unresolved` and must be excluded from personalization signals until corrected.
- Manual rerun command: `php artisan app:backfill-topic-ownership`

### Relevant Test Commands

```bash
php artisan test --testdox
php artisan test --filter=AdminRecommendedProjectManagementTest
php artisan test --filter=HomepageRecommendationApiTest
php artisan test --filter=Phase7EndToEndVerificationTest
```

## About Laravel

Laravel is a web application framework with expressive, elegant syntax. We believe development must be an enjoyable and creative experience to be truly fulfilling. Laravel takes the pain out of development by easing common tasks used in many web projects, such as:

- [Simple, fast routing engine](https://laravel.com/docs/routing).
- [Powerful dependency injection container](https://laravel.com/docs/container).
- Multiple back-ends for [session](https://laravel.com/docs/session) and [cache](https://laravel.com/docs/cache) storage.
- Expressive, intuitive [database ORM](https://laravel.com/docs/eloquent).
- Database agnostic [schema migrations](https://laravel.com/docs/migrations).
- [Robust background job processing](https://laravel.com/docs/queues).
- [Real-time event broadcasting](https://laravel.com/docs/broadcasting).

Laravel is accessible, powerful, and provides tools required for large, robust applications.

## Learning Laravel

Laravel has the most extensive and thorough [documentation](https://laravel.com/docs) and video tutorial library of all modern web application frameworks, making it a breeze to get started with the framework.

In addition, [Laracasts](https://laracasts.com) contains thousands of video tutorials on a range of topics including Laravel, modern PHP, unit testing, and JavaScript. Boost your skills by digging into our comprehensive video library.

You can also watch bite-sized lessons with real-world projects on [Laravel Learn](https://laravel.com/learn), where you will be guided through building a Laravel application from scratch while learning PHP fundamentals.

## Agentic Development

Laravel's predictable structure and conventions make it ideal for AI coding agents like Claude Code, Cursor, and GitHub Copilot. Install [Laravel Boost](https://laravel.com/docs/ai) to supercharge your AI workflow:

```bash
composer require laravel/boost --dev

php artisan boost:install
```

Boost provides your agent 15+ tools and skills that help agents build Laravel applications while following best practices.

## Contributing

Thank you for considering contributing to the Laravel framework! The contribution guide can be found in the [Laravel documentation](https://laravel.com/docs/contributions).

## Code of Conduct

In order to ensure that the Laravel community is welcoming to all, please review and abide by the [Code of Conduct](https://laravel.com/docs/contributions#code-of-conduct).

## Security Vulnerabilities

If you discover a security vulnerability within Laravel, please send an e-mail to Taylor Otwell via [taylor@laravel.com](mailto:taylor@laravel.com). All security vulnerabilities will be promptly addressed.

## License

The Laravel framework is open-sourced software licensed under the [MIT license](https://opensource.org/licenses/MIT).
