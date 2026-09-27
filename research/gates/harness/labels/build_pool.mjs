// Build unlabelled candidate pools for the WP-4 gate label sets from real text.
//
//   node build_pool.mjs --steve <steve_rows.jsonl> --research <research dir>
//        --soak <soak journal.jsonl> --out <pool dir>
//
// `steve_rows.jsonl` is a read-only dump of Steve's recovered memory volume
// (`docker cp ferricula-steve:/data/agent-memory`, read with
// ferricula-core DurableEngine; one {"id", "tags"} object per row). Only the
// `text` tag is used (a <= 200-char preview in that volume).
//
// Output (all deterministic for fixed inputs):
//   worth_researching.pool.jsonl  {id, state, kind, source, group}
//   vedana.pool.jsonl             {id, text, kind, source, group}
//   sati_sources.jsonl            {id, text, kind, source, group}   memories for query generation
//   real_queries.jsonl            {id, query, source, group}        operator questions from Steve's volume
import fs from 'node:fs';
import path from 'node:path';
import { args, readJsonl, writeJsonl, unit, looksSensitive, sha256 } from './common.mjs';

const a = args();
for (const k of ['steve', 'research', 'soak', 'out']) if (!a[k]) throw new Error(`--${k} required`);

const clean = (s) => s.replace(/\s+/g, ' ').trim();
const seen = new Set();
const fresh = (t) => {
  const k = t.toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim();
  if (!k || seen.has(k)) return false;
  seen.add(k);
  return true;
};

// ---- Steve's memory volume
const steve = { heard: [], said: [], thought: [], seeing: [], crawl: [] };
for (const r of readJsonl(a.steve)) {
  const t = r.tags || {};
  let text = t.text || '';
  if (t.type === 'confer' || t.type === 'becoming') continue; // meta logs of the inner critic
  if (t.channel === 'hearing' && text.startsWith('<!--')) {
    text = text.replace(/<!--[\s\S]*?-->/g, '').replace(/^#+\s*/gm, '');
  }
  text = clean(text);
  let kind;
  if (text.startsWith('[heard]')) { kind = 'heard'; text = clean(text.slice(7)); }
  else if (text.startsWith('[said]')) { kind = 'said'; text = clean(text.slice(6)); }
  else if (t.channel === 'hearing') kind = 'crawl';
  else if (t.channel === 'seeing') kind = 'seeing';
  else if (t.channel === 'thinking') kind = 'thought';
  else continue;
  if (text.length < 5 || text === '...' || looksSensitive(text) || !fresh(text)) continue;
  if (kind === 'crawl' && text.length < 80) continue;
  steve[kind].push({ id: `steve:${r.id}`, text, kind: `steve-${kind}`, source: `steve-memory ${kind} row ${r.id}`, group: `steve:${r.id}` });
}

// ---- research corpus paragraphs (top-level research/*.md only)
const research = [];
for (const f of fs.readdirSync(a.research).filter((f) => f.endsWith('.md')).sort()) {
  if (/^(INDEX|REFERENCES|RESEARCH_RUN)\.md$/.test(f)) continue;
  const body = fs.readFileSync(path.join(a.research, f), 'utf8');
  let inCode = false, n = 0;
  for (const para of body.split(/\n\s*\n/)) {
    const fences = para.split('```').length - 1;
    if (fences) { if (fences % 2) inCode = !inCode; continue; }
    if (inCode) continue;
    const p = para.trim();
    if (!p || /^(#|\||>|<|!\[|\$\$|---)/.test(p)) continue;
    const text = clean(p.replace(/\*\*|__|`/g, '').replace(/\[([^\]]+)\]\([^)]+\)/g, '$1'));
    n++;
    if (text.length < 150 || text.length > 700 || looksSensitive(text) || !fresh(text)) continue;
    research.push({ id: `research:${f}#${n}`, text, kind: 'research-paragraph', source: `research corpus ${f} paragraph ${n}`, group: `research:${f}` });
  }
}

// ---- soak journal (operator thread seeds and reflections)
const soak = [];
for (const r of readJsonl(a.soak)) {
  const ts = r.ts || 0;
  const topic = r.seed?.topic;
  if (typeof topic === 'string' && topic.length > 20 && fresh(topic)) {
    soak.push({ id: `soak:${ts}:seed`, text: clean(topic), kind: 'soak-thread', source: `soak journal ${ts} curiosity seed`, group: `soak:${ts}` });
  }
  if (typeof r.reflection === 'string' && fresh(r.reflection)) {
    soak.push({ id: `soak:${ts}:reflection`, text: clean(r.reflection), kind: 'soak-reflection', source: `soak journal ${ts} reflection`, group: `soak:${ts}` });
  }
}

// Deterministic sample of k items (salted so each gate draws its own).
const take = (list, k, salt) => [...list].sort((x, y) => unit(`${salt}|${x.id}`) - unit(`${salt}|${y.id}`)).slice(0, k);
const stats = Object.fromEntries(Object.entries(steve).map(([k, v]) => [k, v.length]));
console.error('available', { ...stats, research: research.length, soak: soak.length });

// Curiosity gate state mirrors the runtime: an operator turn as "Operator: ...",
// or a memory text as drawn by the entropy path.
const asOperator = (x) => ({ ...x, state: x.text.startsWith('Operator:') ? x.text : `Operator: ${x.text}` });
const worth = [
  ...take(steve.heard, 220, 'worth').map(asOperator),
  ...take(steve.thought, 110, 'worth'),
  ...take(steve.said, 50, 'worth'),
  ...take(steve.seeing, 30, 'worth'),
  ...take(steve.crawl, 60, 'worth'),
  ...take(research, 170, 'worth'),
  ...soak.filter((s) => s.kind === 'soak-thread').map(asOperator),
].map((x) => ({ id: `wr:${x.id}`, state: x.state || x.text, kind: x.kind, source: x.source, group: x.group }));

const vedana = [
  ...take(steve.heard, 170, 'vedana'),
  ...take(steve.thought, 170, 'vedana'),
  ...take(steve.said, 60, 'vedana'),
  ...take(steve.seeing, 50, 'vedana'),
  ...take(steve.crawl, 50, 'vedana'),
  ...take(research, 110, 'vedana'),
  ...soak,
].map((x) => ({ id: `ve:${x.id}`, text: x.text, kind: x.kind, source: x.source, group: x.group }));

// Recall sources: informative memories long enough to answer something.
const informative = (x) => x.text.length >= 100;
const sati = [
  ...take(steve.thought.filter(informative), 110, 'sati'),
  ...take(steve.crawl.filter(informative), 40, 'sati'),
  ...take(steve.said.filter(informative), 20, 'sati'),
  ...take(research, 110, 'sati'),
];

const isQuestion = (t) => /\?\s*$/.test(t) || /^(what|who|whom|why|when|where|how|which|do|does|did|is|are|can|could|would|should|have|has)\b/i.test(t);
const realQueries = take(steve.heard.filter((x) => isQuestion(x.text) && x.text.split(' ').length >= 4), 120, 'queries')
  .map((x) => ({ id: x.id, query: x.text, source: x.source, group: x.group }));

writeJsonl(path.join(a.out, 'worth_researching.pool.jsonl'), worth);
writeJsonl(path.join(a.out, 'vedana.pool.jsonl'), vedana);
writeJsonl(path.join(a.out, 'sati_sources.jsonl'), sati);
writeJsonl(path.join(a.out, 'real_queries.jsonl'), realQueries);
const manifest = {
  inputs: { steve_rows_sha256: sha256(fs.readFileSync(a.steve)), soak_sha256: sha256(fs.readFileSync(a.soak)) },
  available: { ...stats, research: research.length, soak: soak.length },
  pools: { worth_researching: worth.length, vedana: vedana.length, sati_sources: sati.length, real_queries: realQueries.length },
};
fs.writeFileSync(path.join(a.out, 'pool_manifest.json'), JSON.stringify(manifest, null, 2));
console.log(JSON.stringify(manifest, null, 2));
