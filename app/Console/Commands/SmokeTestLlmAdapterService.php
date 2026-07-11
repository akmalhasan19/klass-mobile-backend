<?php

namespace App\Console\Commands;

use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationServiceException;
use App\Services\LlmAdapterHealthCheckService;
use App\Services\LlmAdapterSmokeTestService;
use Illuminate\Console\Attributes\Description;
use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;

#[Signature('media-generation:smoke-llm-adapter
    {--allow-unconfigured-auth : Do not fail when the adapter health payload reports auth.configured=false or auth.ready=false}
    {--exercise-routes : Send signed smoke requests to /v1/interpret and /v1/respond using the backend adapter contract}
    {--expect-provider= : Fail if the exercised smoke routes report a provider different from this alias}')] 
#[Description('Smoke test reachability, readiness, and optional signed route execution for the LLM adapter service')]
class SmokeTestLlmAdapterService extends Command
{
    public function handle(
        LlmAdapterHealthCheckService $healthCheckService,
        LlmAdapterSmokeTestService $smokeTestService,
    ): int {
        $requireAuthConfigured = ! (bool) $this->option('allow-unconfigured-auth');

        try {
            $payloads = $healthCheckService->check($requireAuthConfigured);
        } catch (MediaGenerationServiceException|MediaGenerationContractException $exception) {
            $this->error($exception->getMessage());
            $this->renderContext($exception->context());

            return self::FAILURE;
        }

        $healthPayload = $payloads['health'];

        $this->info('LLM adapter service is reachable and healthy.');
        $this->line('Service: ' . trim((string) data_get($healthPayload, 'service_name', 'unknown')));
        $this->line('Version: ' . trim((string) data_get($healthPayload, 'service_version', 'unknown')));
        $this->line('Health paths: /health, /' . ltrim((string) config('services.media_generation.llm_adapter.health_path', '/v1/health'), '/'));
        $this->line('Postgres ready: ' . (data_get($healthPayload, 'dependencies.postgres.ready') === true ? 'yes' : 'no'));
        $this->line('Interpretation provider: ' . trim((string) data_get($healthPayload, 'dependencies.providers.interpretation.provider', 'unknown')));
        $this->line('Delivery provider: ' . trim((string) data_get($healthPayload, 'dependencies.providers.delivery.provider', 'unknown')));
        $this->line('Auth configured: ' . (data_get($healthPayload, 'auth.configured') === true ? 'yes' : 'no'));
        $this->line('Rotation enabled: ' . (data_get($healthPayload, 'auth.rotation_enabled') === true ? 'yes' : 'no'));

        if (! (bool) $this->option('exercise-routes')) {
            return self::SUCCESS;
        }

        try {
            $result = $smokeTestService->exerciseRoutes((string) $this->option('expect-provider'));
        } catch (MediaGenerationServiceException|MediaGenerationContractException $exception) {
            $this->error($exception->getMessage());
            $this->renderContext($exception->context());

            return self::FAILURE;
        }

        $this->line('Interpret smoke provider: ' . $result['interpret']['provider']);
        $this->line('Interpret smoke model: ' . $result['interpret']['model']);
        $this->line('Respond smoke provider: ' . $result['respond']['provider']);
        $this->line('Respond smoke model: ' . $result['respond']['model']);

        return self::SUCCESS;
    }

    /**
     * @param  array<string, mixed>  $context
     */
    protected function renderContext(array $context): void
    {
        foreach ($context as $key => $value) {
            if (is_array($value)) {
                $this->line($key . ': ' . json_encode($value, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES));

                continue;
            }

            $this->line($key . ': ' . (is_bool($value) ? ($value ? 'true' : 'false') : (string) $value));
        }
    }
}