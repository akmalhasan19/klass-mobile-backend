import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Trend, Rate, Counter } from 'k6/metrics';

const e2eDuration = new Trend('media_gen_e2e_duration', true);
const submitToFirstPollDuration = new Trend('submit_to_first_poll_duration', true);
const pollCount = new Counter('poll_requests');
const errorRate = new Rate('errors');

const BASE_URL = __ENV.API_BASE_URL || 'http://localhost:8000';
const TEACHER_EMAIL = __ENV.TEST_TEACHER_EMAIL || 'teacher@example.com';
const TEACHER_PASSWORD = __ENV.TEST_TEACHER_PASSWORD || 'password123';
const MAX_POLL_SECONDS = parseInt(__ENV.MAX_POLL_SECONDS || '300');
const POLL_INTERVAL_MS = parseInt(__ENV.POLL_INTERVAL_MS || '4000');

export const options = {
  scenarios: {
    e2e_single: {
      executor: 'constant-vus',
      vus: 1,
      duration: '10m',
    },
  },
  thresholds: {
    media_gen_e2e_duration: ['p(95)<180000'],
    errors: ['rate<0.1'],
  },
};

function getAuthToken() {
  const res = http.post(`${BASE_URL}/api/v1/auth/login`, JSON.stringify({
    email: TEACHER_EMAIL,
    password: TEACHER_PASSWORD,
  }), {
    headers: { 'Content-Type': 'application/json', 'Accept': 'application/json' },
  });

  if (res.status === 200) {
    return JSON.parse(res.body).data?.token || '';
  }
  return '';
}

export default function () {
  const token = getAuthToken();
  if (!token) {
    console.error('Failed to get auth token');
    return;
  }

  const headers = {
    'Content-Type': 'application/json',
    'Accept': 'application/json',
    'Authorization': `Bearer ${token}`,
  };

  group('media_generation_e2e', function () {
    const submitStart = Date.now();

    const submitRes = http.post(`${BASE_URL}/api/v1/media-generations`, JSON.stringify({
      prompt: 'Buatkan materi pembelajaran tentang fotosintesis untuk kelas 5 SD dalam format PDF',
      preferred_output_type: 'pdf',
    }), { headers });

    check(submitRes, {
      'submit accepted (202)': (r) => r.status === 202,
      'submit has generation id': (r) => JSON.parse(r.body).data?.id !== undefined,
    });

    if (submitRes.status !== 202) {
      errorRate.add(1);
      return;
    }

    const generationId = JSON.parse(submitRes.body).data.id;
    let isTerminal = false;
    let lastStatus = 'queued';
    const maxPolls = Math.ceil((MAX_POLL_SECONDS * 1000) / POLL_INTERVAL_MS);

    for (let i = 0; i < maxPolls; i++) {
      sleep(POLL_INTERVAL_MS / 1000);

      const pollRes = http.get(`${BASE_URL}/api/v1/media-generations/${generationId}`, { headers });
      pollCount.add(1);

      if (pollRes.status === 200) {
        const body = JSON.parse(pollRes.body);
        lastStatus = body.data?.status || 'unknown';
        isTerminal = body.data?.status_meta?.is_terminal || false;

        if (isTerminal) {
          break;
        }
      }
    }

    const e2eMs = Date.now() - submitStart;
    e2eDuration.add(e2eMs);

    const isCompleted = lastStatus === 'completed';
    errorRate.add(!isCompleted);

    check(null, {
      'generation completed': () => isCompleted,
      [`final status: ${lastStatus}`]: () => true,
      [`e2e duration: ${(e2eMs / 1000).toFixed(1)}s`]: () => true,
    });

    console.log(`Generation ${generationId}: ${lastStatus} in ${(e2eMs / 1000).toFixed(1)}s`);
  });
}
