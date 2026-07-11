<?php

namespace App\Console\Commands;

use App\MediaGeneration\MediaGenerationContractException;
use App\MediaGeneration\MediaGenerationServiceException;
use App\Services\PythonMediaGeneratorHealthCheckService;
use Illuminate\Console\Attributes\Description;
use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;

#[Signature('media-generation:smoke-python-service {--allow-unconfigured-auth : Do not fail when the Python health payload reports auth.configured=false}')]
#[Description('Smoke test reachability and contract health for the Python media generator service')]
class SmokeTestMediaGeneratorService extends Command
{
    public function handle(PythonMediaGeneratorHealthCheckService $healthCheckService): int
    {
        $requireAuthConfigured = ! (bool) $this->option('allow-unconfigured-auth');

        try {
            $payload = $healthCheckService->check($requireAuthConfigured);
        } catch (MediaGenerationServiceException|MediaGenerationContractException $exception) {
            $this->error($exception->getMessage());
            $this->renderContext($exception->context());

            return self::FAILURE;
        }

        $this->info('Python media generator service is reachable and healthy.');
        $this->line('Service: ' . trim((string) data_get($payload, 'service', 'unknown')));
        $this->line('Version: ' . trim((string) data_get($payload, 'version', 'unknown')));
        $this->line('Health path: /' . ltrim((string) config('services.media_generation.python.health_path', '/v1/health'), '/'));
        $this->line('Supported formats: ' . implode(', ', (array) data_get($payload, 'supported_formats', [])));
        $this->line('Auth configured: ' . (data_get($payload, 'auth.configured') === true ? 'yes' : 'no'));
        $this->line('Rotation enabled: ' . (data_get($payload, 'auth.rotation_enabled') === true ? 'yes' : 'no'));

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