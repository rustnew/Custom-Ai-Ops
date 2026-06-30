import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
    stages: [
        { duration: '30s', target: 5 },
        { duration: '2m', target: 10 },
        { duration: '5m', target: 20 },
        { duration: '2m', target: 10 },
        { duration: '30s', target: 0 },
    ],
    thresholds: {
        http_req_duration: ['p(95)<2000'],
        http_req_failed: ['rate<0.05'],
    },
};

const BASE_URL = __ENV.MODEL_URL || 'http://localhost:8000';
const API_KEY = __ENV.API_KEY || '';
const MODEL_NAME = __ENV.MODEL_NAME || 'test';

export default function () {
    const headers = {
        'Content-Type': 'application/json',
    };
    if (API_KEY) {
        headers['Authorization'] = `Bearer ${API_KEY}`;
    }

    const payload = JSON.stringify({
        model: MODEL_NAME,
        messages: [
            { role: 'user', content: 'Write a short paragraph about artificial intelligence.' },
        ],
        max_tokens: 128,
        temperature: 0.7,
    });

    const res = http.post(`${BASE_URL}/v1/chat/completions`, payload, { headers });

    check(res, {
        'status is 200': (r) => r.status === 200,
        'has choices': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.choices && body.choices.length > 0;
            } catch {
                return false;
            }
        },
        'latency < 2s': (r) => r.timings.duration < 2000,
    });

    sleep(1);
}