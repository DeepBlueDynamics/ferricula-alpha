// Merge two annotators' raw labels into an agreement-only dataset.
//
//   node merge.mjs --pool <pool dir> --raw <raw dir> --out <datasets dir> --stats <stats.json>
//        [--models glm-5.3:cloud,kimi-k3:cloud]
//
// Keeps an item only when both annotators gave the same primary label
// (worth / valence / answers). Split is fixed by group hash (common.splitOf).
import fs from 'node:fs';
import path from 'node:path';
import { args, readJsonl, writeJsonl, splitOf, sha256File } from './common.mjs';

const a = args();
for (const k of ['pool', 'raw', 'out', 'stats']) if (!a[k]) throw new Error(`--${k} required`);
const models = (a.models || 'glm-5.3:cloud,kimi-k3:cloud').split(',');
const slug = (m) => m.replace(/[^a-z0-9.-]/gi, '_');

const GATES = {
  worth_researching: {
    primary: 'worth',
    classes: [true, false],
    row: (p, l1, l2) => ({ id: p.id, state: p.state, worth: l1.worth, label: l1.worth ? 'yes' : 'no' }),
  },
  vedana: {
    primary: 'valence',
    classes: ['sukha', 'dukkha', 'adukkhamasukha'],
    row: (p, l1, l2) => ({
      id: p.id, text: p.text, valence: l1.valence,
      intensity: Math.abs(l1.intensity - l2.intensity) <= 1 ? (l1.intensity + l2.intensity) / 2 : null,
      intensity_annotators: [l1.intensity, l2.intensity],
    }),
  },
  sati_recall: {
    primary: 'answers',
    classes: [true, false],
    row: (p, l1, l2) => ({
      id: p.id, query: p.query, memory: p.memory, answers: l1.answers, label: l1.answers ? 'yes' : 'no',
      relevance: (l1.relevance + l2.relevance) / 2, construction: p.construction, memory_id: p.memory_id,
    }),
  },
};

function kappa(pairs, classes) {
  const n = pairs.length;
  if (!n) return NaN;
  const po = pairs.filter(([x, y]) => x === y).length / n;
  let pe = 0;
  for (const c of classes) {
    pe += (pairs.filter(([x]) => x === c).length / n) * (pairs.filter(([, y]) => y === c).length / n);
  }
  return (po - pe) / (1 - pe);
}

const stats = { annotators: models, datasets: {} };
for (const [gate, spec] of Object.entries(GATES)) {
  const poolPath = path.join(a.pool, `${gate}.pool.jsonl`);
  if (!fs.existsSync(poolPath)) continue;
  const pool = readJsonl(poolPath);
  const raw = models.map((m) => new Map(readJsonl(path.join(a.raw, `${gate}.${slug(m)}.jsonl`)).map((r) => [r.id, r])));
  const both = pool.filter((p) => raw[0].has(p.id) && raw[1].has(p.id));
  const pairs = both.map((p) => [raw[0].get(p.id)[spec.primary], raw[1].get(p.id)[spec.primary]]);
  const kept = [];
  const byKind = {};
  for (const p of both) {
    const [l1, l2] = [raw[0].get(p.id), raw[1].get(p.id)];
    const agree = l1[spec.primary] === l2[spec.primary];
    const k = (byKind[p.kind || p.construction] ||= { n: 0, agree: 0 });
    k.n++;
    if (!agree) continue;
    k.agree++;
    kept.push({
      ...spec.row(p, l1, l2),
      split: splitOf(p.group),
      lang: 'en',
      kind: p.kind || p.construction,
      source: p.source,
      group: p.group,
      label_provenance: `llm-agreement: ${models.join(' + ')} (temperature 0); not human-labelled`,
    });
  }
  const out = path.join(a.out, `${gate}_en.v3.jsonl`);
  writeJsonl(out, kept);
  const bal = {};
  for (const r of kept) {
    const c = String(r[spec.primary]);
    (bal[r.split] ||= {})[c] = (bal[r.split][c] || 0) + 1;
  }
  const intensityNull = gate === 'vedana' ? kept.filter((r) => r.intensity === null).length : undefined;
  stats.datasets[gate] = {
    file: out.replace(/\\/g, '/'),
    sha256: sha256File(out),
    pool: pool.length,
    labelled_by_both: both.length,
    agreed: kept.length,
    agreement_rate: +(kept.length / both.length).toFixed(4),
    cohen_kappa: +kappa(pairs, spec.classes).toFixed(4),
    per_kind: byKind,
    split_label_counts: bal,
    ...(intensityNull !== undefined ? { intensity_disagreement_gt1: intensityNull } : {}),
    prompt_sha256: sha256File(new URL(`../../prompts/annotate_${gate}.txt`, import.meta.url)),
  };
}
fs.writeFileSync(a.stats, JSON.stringify(stats, null, 2) + '\n');
console.log(JSON.stringify(stats, null, 2));
