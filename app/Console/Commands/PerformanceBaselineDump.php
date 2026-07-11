<?php

namespace App\Console\Commands;

use App\Models\MediaGeneration;
use Illuminate\Console\Command;

class PerformanceBaselineDump extends Command
{
    protected $signature = 'perf:baseline
        {--days=7 : Number of days to analyze}
        {--output=table : Output format: table, json, csv}';

    protected $description = 'Dump performance baseline metrics from media_generations audit trail';

    public function handle(): int
    {
        $days = (int) $this->option('days');
        $output = $this->option('output');

        $this->info("=== Performance Baseline (last {$days} days) ===\n");

        $this->dumpE2ELatency($days);
        $this->dumpStatusDistribution($days);
        $this->dumpErrorRates($days);
        $this->dumpThroughput($days);
        $this->dumpOutputTypeBreakdown($days);

        return self::SUCCESS;
    }

    protected function dumpE2ELatency(int $days): void
    {
        $this->info('--- E2E Latency (total_duration_ms) ---');

        $generations = MediaGeneration::query()
            ->whereNotNull('orchestration_audit_payload->timing->total_duration_ms')
            ->where('created_at', '>=', now()->subDays($days))
            ->get()
            ->map(fn (MediaGeneration $g) => (float) data_get($g->orchestration_audit_payload, 'timing.total_duration_ms', 0))
            ->filter()
            ->sort()
            ->values();

        if ($generations->isEmpty()) {
            $this->warn("  No completed generations found in the last {$days} days.\n");
            return;
        }

        $count = $generations->count();
        $p50 = $generations[(int) ($count * 0.5)] ?? 0;
        $p95 = $generations[(int) ($count * 0.95)] ?? 0;
        $p99 = $generations[(int) ($count * 0.99)] ?? 0;
        $avg = $generations->avg();
        $max = $generations->max();
        $min = $generations->min();

        $this->table(
            ['Metric', 'Value'],
            [
                ['Sample Size', $count],
                ['p50', number_format($p50, 0) . ' ms (' . number_format($p50 / 1000, 1) . 's)'],
                ['p95', number_format($p95, 0) . ' ms (' . number_format($p95 / 1000, 1) . 's)'],
                ['p99', number_format($p99, 0) . ' ms (' . number_format($p99 / 1000, 1) . 's)'],
                ['Average', number_format($avg, 0) . ' ms (' . number_format($avg / 1000, 1) . 's)'],
                ['Max', number_format($max, 0) . ' ms (' . number_format($max / 1000, 1) . 's)'],
                ['Min', number_format($min, 0) . ' ms (' . number_format($min / 1000, 1) . 's)'],
            ]
        );

        $this->newLine();
    }

    protected function dumpStatusDistribution(int $days): void
    {
        $this->info('--- Status Distribution ---');

        $stats = MediaGeneration::query()
            ->where('created_at', '>=', now()->subDays($days))
            ->selectRaw('status, COUNT(*) as count')
            ->groupBy('status')
            ->orderByDesc('count')
            ->get();

        $total = $stats->sum('count');

        $rows = $stats->map(fn ($row) => [
            $row->status,
            $row->count,
            number_format($row->count * 100 / max($total, 1), 1) . '%',
        ])->toArray();

        $this->table(['Status', 'Count', 'Percentage'], $rows);
        $this->newLine();
    }

    protected function dumpErrorRates(int $days): void
    {
        $this->info('--- Error Rates (last ' . $days . ' days) ---');

        $errors = MediaGeneration::query()
            ->whereNotNull('error_code')
            ->where('created_at', '>=', now()->subDays($days))
            ->selectRaw('error_code, COUNT(*) as count')
            ->groupBy('error_code')
            ->orderByDesc('count')
            ->get();

        if ($errors->isEmpty()) {
            $this->info("  No errors found.\n");
            return;
        }

        $total = $errors->sum('count');

        $rows = $errors->map(fn ($row) => [
            $row->error_code,
            $row->count,
            number_format($row->count * 100 / max($total, 1), 1) . '%',
        ])->toArray();

        $this->table(['Error Code', 'Count', 'Percentage'], $rows);
        $this->newLine();
    }

    protected function dumpThroughput(int $days): void
    {
        $this->info('--- Throughput (generations/hour, last ' . $days . ' days) ---');

        $hourly = MediaGeneration::query()
            ->where('created_at', '>=', now()->subDays($days))
            ->selectRaw("date_trunc('hour', created_at) as hour, COUNT(*) as submitted, COUNT(*) FILTER (WHERE status = 'completed') as completed, COUNT(*) FILTER (WHERE status = 'failed') as failed")
            ->groupByRaw("date_trunc('hour', created_at)")
            ->orderByDesc('hour')
            ->limit(24)
            ->get();

        if ($hourly->isEmpty()) {
            $this->warn("  No data found.\n");
            return;
        }

        $rows = $hourly->map(fn ($row) => [
            $row->hour?->format('Y-m-d H:00'),
            $row->submitted,
            $row->completed,
            $row->failed,
        ])->toArray();

        $this->table(['Hour', 'Submitted', 'Completed', 'Failed'], $rows);
        $this->newLine();
    }

    protected function dumpOutputTypeBreakdown(int $days): void
    {
        $this->info('--- Output Type Breakdown ---');

        $types = MediaGeneration::query()
            ->whereNotNull('resolved_output_type')
            ->where('created_at', '>=', now()->subDays($days))
            ->selectRaw('resolved_output_type, COUNT(*) as count')
            ->groupBy('resolved_output_type')
            ->orderByDesc('count')
            ->get();

        if ($types->isEmpty()) {
            $this->warn("  No data found.\n");
            return;
        }

        $this->table(
            ['Output Type', 'Count'],
            $types->map(fn ($row) => [$row->resolved_output_type, $row->count])->toArray()
        );
        $this->newLine();
    }
}
