// Shared helpers for the WP-4 label harvest (build_pool / annotate / merge).
// Node >= 20, no dependencies.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

export const sha256 = (s) => crypto.createHash('sha256').update(s).digest('hex');
export const sha256File = (p) => sha256(fs.readFileSync(p));

/// Deterministic [0,1) from a string.
export const unit = (s) => parseInt(sha256(s).slice(0, 8), 16) / 0x100000000;

/// Fixed split by group: 40% train, 20% dev, 40% test (test is large so a
/// held-out ECE can reach n >= 100).
export function splitOf(group) {
  const u = unit(`wp4-split|${group}`);
  return u < 0.4 ? 'train' : u < 0.6 ? 'dev' : 'test';
}

export function readJsonl(p) {
  if (!fs.existsSync(p)) return [];
  return fs.readFileSync(p, 'utf8').split('\n').filter((l) => l.trim()).map((l) => JSON.parse(l));
}

export function writeJsonl(p, rows) {
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, rows.map((r) => JSON.stringify(r)).join('\n') + (rows.length ? '\n' : ''));
}

export function args(argv = process.argv.slice(2)) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) throw new Error(`unexpected argument ${a}`);
    const v = argv[i + 1];
    if (v === undefined || v.startsWith('--')) out[a.slice(2)] = true;
    else { out[a.slice(2)] = v; i++; }
  }
  return out;
}

const STOP = new Set(('a an the and or but if of to in on at by for with from as is are was were be been being it its this that these those ' +
  'i you he she we they me him her us them my your his our their what which who whom how why when where do does did not no yes ' +
  'can could would should will shall may might must have has had so than then there here just about into over under also very ' +
  'more most some any all each other such only own same too s t don now').split(' '));

export function tokens(s) {
  return (s.toLowerCase().normalize('NFKD').replace(/[̀-ͯ]/g, '').match(/[a-z0-9]+/g) || [])
    .filter((t) => t.length > 2 && !STOP.has(t));
}

/// TF-IDF cosine index over a list of texts.
export function tfidfIndex(texts) {
  const df = new Map();
  const docs = texts.map((t) => {
    const tf = new Map();
    for (const w of tokens(t)) tf.set(w, (tf.get(w) || 0) + 1);
    for (const w of tf.keys()) df.set(w, (df.get(w) || 0) + 1);
    return tf;
  });
  const n = texts.length;
  const vec = (tf) => {
    const v = new Map();
    let norm = 0;
    for (const [w, c] of tf) {
      const x = (1 + Math.log(c)) * Math.log((n + 1) / ((df.get(w) || 0) + 1));
      v.set(w, x);
      norm += x * x;
    }
    return { v, norm: Math.sqrt(norm) || 1 };
  };
  const vecs = docs.map(vec);
  return {
    query(text) {
      const tf = new Map();
      for (const w of tokens(text)) tf.set(w, (tf.get(w) || 0) + 1);
      const q = vec(tf);
      return vecs.map((d, i) => {
        let dot = 0;
        for (const [w, x] of q.v) { const y = d.v.get(w); if (y) dot += x * y; }
        return [i, dot / (q.norm * d.norm)];
      }).sort((a, b) => b[1] - a[1]);
    },
  };
}

/// Drop anything that looks like a credential or personal contact data.
export function looksSensitive(s) {
  return /(sk-[A-Za-z0-9]{8,}|ghp_[A-Za-z0-9]{8,}|AKIA[0-9A-Z]{12,}|api[_-]?key|password|passwd|secret|bearer\s|token=|ahp_[A-Za-z0-9])/i.test(s)
    || /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/.test(s)
    || /\+?\d[\d\s().-]{8,}\d/.test(s.replace(/\b(19|20)\d\d\b/g, ''));
}

export async function chat({ url, model, prompt, maxTokens = 16000, timeoutMs = 600000 }) {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const r = await fetch(`${url.replace(/\/$/, '')}/chat/completions`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ model, temperature: 0, max_tokens: maxTokens, stream: false, messages: [{ role: 'user', content: prompt }] }),
      signal: ctl.signal,
    });
    const body = await r.text();
    if (!r.ok) throw new Error(`http ${r.status}: ${body.slice(0, 200)}`);
    const j = JSON.parse(body);
    return { text: j.choices?.[0]?.message?.content || '', usage: j.usage || null, model: j.model || model };
  } finally {
    clearTimeout(timer);
  }
}

/// First balanced {...} object in a completion.
export function firstJson(s) {
  const start = s.indexOf('{');
  if (start < 0) return null;
  let depth = 0, inStr = false, esc = false;
  for (let i = start; i < s.length; i++) {
    const c = s[i];
    if (inStr) { if (esc) esc = false; else if (c === '\\') esc = true; else if (c === '"') inStr = false; continue; }
    if (c === '"') inStr = true;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) { try { return JSON.parse(s.slice(start, i + 1)); } catch { return null; } }
  }
  return null;
}

export async function pool(items, concurrency, fn) {
  let next = 0;
  const workers = Array.from({ length: concurrency }, async () => {
    while (next < items.length) { const i = next++; await fn(items[i], i); }
  });
  await Promise.all(workers);
}
