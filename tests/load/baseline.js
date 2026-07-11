import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

const errorRate = new Rate('errors');
const loginDuration = new Trend('login_duration', true);
const registerDuration = new Trend('register_duration', true);
const topicsDuration = new Trend('topics_duration', true);
const contentsDuration = new Trend('contents_duration', true);
const mediaGenListDuration = new Trend('media_gen_list_duration', true);
const mediaGenSubmitDuration = new Trend('media_gen_submit_duration', true);
const mediaGenShowDuration = new Trend('media_gen_show_duration', true);
const requestCount = new Counter('total_requests');

const BASE_URL = __ENV.API_BASE_URL || 'http://localhost:8000';
const TEACHER_EMAIL = __ENV.TEST_TEACHER_EMAIL || 'teacher@example.com';
const TEACHER_PASSWORD = __ENV.TEST_TEACHER_PASSWORD || 'password123';

export const options = {
  scenarios: {
    smoke: {
      executor: 'constant-vus',
      vus: 1,
      duration: '30s',
      startTime: '0s',
    },
    load_10: {
      executor: 'constant-vus',
      vus: 10,
      duration: '2m',
      startTime: '30s',
    },
    load_50: {
      executor: 'constant-vus',
      vus: 50,
      duration: '3m',
      startTime: '2m30s',
    },
    load_100: {
      executor: 'constant-vus',
      vus: 100,
      duration: '3m',
      startTime: '5m30s',
    },
    spike: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 200 },
        { duration: '1m', target: 200 },
        { duration: '30s', target: 0 },
      ],
      startTime: '8m30s',
    },
  },
  thresholds: {
    http_req_duration: ['p(95)<800', 'p(99)<2000'],
    errors: ['rate<0.05'],
    login_duration: ['p(95)<500'],
    topics_duration: ['p(95)<300'],
    media_gen_submit_duration: ['p(95)<1000'],
  },
};

function getAuthToken() {
  const loginRes = http.post(`${BASE_URL}/api/v1/auth/login`, JSON.stringify({
    email: TEACHER_EMAIL,
    password: TEACHER_PASSWORD,
  }), {
    headers: { 'Content-Type': 'application/json', 'Accept': 'application/json' },
  });

  loginDuration.add(loginRes.timings.duration);
  errorRate.add(loginRes.status !== 200);
  requestCount.add(1);

  if (loginRes.status === 200) {
    const body = JSON.parse(loginRes.body);
    return body.data?.token || '';
  }

  return '';
}

export default function () {
  const token = getAuthToken();

  const authHeaders = {
    'Content-Type': 'application/json',
    'Accept': 'application/json',
    ...(token ? { 'Authorization': `Bearer ${token}` } : {}),
  };

  group('public_endpoints', function () {
    const topicsRes = http.get(`${BASE_URL}/api/v1/topics?per_page=15`, { headers: { 'Accept': 'application/json' } });
    topicsDuration.add(topicsRes.timings.duration);
    errorRate.add(topicsRes.status !== 200);
    requestCount.add(1);

    check(topicsRes, {
      'topics status 200': (r) => r.status === 200,
      'topics has data': (r) => JSON.parse(r.body).data !== undefined,
    });

    const contentsRes = http.get(`${BASE_URL}/api/v1/contents?per_page=15`, { headers: { 'Accept': 'application/json' } });
    contentsDuration.add(contentsRes.timings.duration);
    errorRate.add(contentsRes.status !== 200);
    requestCount.add(1);

    const galleryRes = http.get(`${BASE_URL}/api/v1/gallery?per_page=15`, { headers: { 'Accept': 'application/json' } });
    errorRate.add(galleryRes.status !== 200);
    requestCount.add(1);

    const homepageRes = http.get(`${BASE_URL}/api/v1/homepage-sections`, { headers: { 'Accept': 'application/json' } });
    errorRate.add(homepageRes.status !== 200);
    requestCount.add(1);
  });

  if (token) {
    group('authenticated_endpoints', function () {
      const meRes = http.get(`${BASE_URL}/api/v1/auth/me`, { headers: authHeaders });
      errorRate.add(meRes.status !== 200);
      requestCount.add(1);

      check(meRes, {
        'me status 200': (r) => r.status === 200,
        'me has user': (r) => JSON.parse(r.body).data !== undefined,
      });

      const mgListRes = http.get(`${BASE_URL}/api/v1/media-generations`, { headers: authHeaders });
      mediaGenListDuration.add(mgListRes.timings.duration);
      errorRate.add(mgListRes.status !== 200);
      requestCount.add(1);

      const mgSubmitRes = http.post(`${BASE_URL}/api/v1/media-generations`, JSON.stringify({
        prompt: 'Buatkan materi pembelajaran tentang fotosintesis untuk kelas 5 SD',
        preferred_output_type: 'auto',
      }), { headers: authHeaders });

      mediaGenSubmitDuration.add(mgSubmitRes.timings.duration);
      errorRate.add(mgSubmitRes.status !== 202);
      requestCount.add(1);

      check(mgSubmitRes, {
        'media-gen submit 202': (r) => r.status === 202,
        'media-gen has id': (r) => JSON.parse(r.body).data?.id !== undefined,
      });

      if (mgSubmitRes.status === 202) {
        const genId = JSON.parse(mgSubmitRes.body).data.id;

        const showRes = http.get(`${BASE_URL}/api/v1/media-generations/${genId}`, { headers: authHeaders });
        mediaGenShowDuration.add(showRes.timings.duration);
        errorRate.add(showRes.status !== 200);
        requestCount.add(1);

        check(showRes, {
          'media-gen show 200': (r) => r.status === 200,
          'media-gen show has status': (r) => JSON.parse(r.body).data?.status !== undefined,
        });
      }
    });
  }

  sleep(1);
}

export function handleSummary(data) {
  return {
    'tests/load/results/summary.json': JSON.stringify(data, null, 2),
    stdout: textSummary(data, { indent: ' ', enableColors: true }),
  };
}

function textSummary(data) {
  const metrics = data.metrics;
  let summary = '\n=== Performance Baseline Summary ===\n\n';

  for (const [name, metric] of Object.entries(metrics)) {
    if (metric.values) {
      summary += `${name}:\n`;
      if (metric.values.avg !== undefined) summary += `  avg: ${metric.values.avg.toFixed(2)}ms\n`;
      if (metric.values['p(50)'] !== undefined) summary += `  p50: ${metric.values['p(50)'].toFixed(2)}ms\n`;
      if (metric.values['p(95)'] !== undefined) summary += `  p95: ${metric.values['p(95)'].toFixed(2)}ms\n`;
      if (metric.values['p(99)'] !== undefined) summary += `  p99: ${metric.values['p(99)'].toFixed(2)}ms\n`;
      if (metric.values.max !== undefined) summary += `  max: ${metric.values.max.toFixed(2)}ms\n`;
      if (metric.values.rate !== undefined) summary += `  rate: ${(metric.values.rate * 100).toFixed(2)}%\n`;
      if (metric.values.count !== undefined) summary += `  count: ${metric.values.count}\n`;
      summary += '\n';
    }
  }

  return summary;
}
