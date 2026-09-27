// Independent LLM annotation of a candidate pool.
//
//   node annotate.mjs --gate worth_researching|vedana|sati_recall --pool <pool dir>
//        --raw <raw dir> [--models glm-5.3:cloud,kimi-k3:cloud]
//        [--url http://127.0.0.1:11434/v1] [--batch 10] [--concurrency 4]
//
// Each annotator sees the same prompt (research/gates/prompts/annotate_<gate>.txt)
// but its own item order and batch composition (salted by model name), at
// temperature 0, with no access to the other annotator's labels. Results are
// appended to <raw>/<gate>.<model>.jsonl and the run resumes from there.
import fs from 'node:fs';
import path from 'node:path';
import { args, readJsonl, chat, firstJson, pool, unit } from './common.mjs';

const a = args();
const gate = a.gate;
const GATES = {
  worth_researching: {
    item: (r) => r.state,
    parse: (l) => (typeof l.worth === 'boolean' ? { worth: l.worth } : null),
  },
  vedana: {
    item: (r) => r.text,
    parse: (l) => (['sukha', 'dukkha', 'adukkhamasukha'].includes(l.valence) && Number.isInteger(l.intensity) && l.intensity >= 0 && l.intensity <= 4
      ? { valence: l.valence, intensity: l.intensity } : null),
  },
  sati_recall: {
    item: (r) => `\nQuery: ${r.query}\nMemory: ${r.memory}`,
    parse: (l) => (typeof l.answers === 'boolean' && Number.isInteger(l.relevance) && l.relevance >= 0 && l.relevance <= 3
      ? { answers: l.answers, relevance: l.relevance } : null),
  },
};
const spec = GATES[gate];
if (!spec || !a.pool || !a.raw) throw new Error('--gate (worth_researching|vedana|sati_recall), --pool and --raw required');
const url = a.url || 'http://127.0.0.1:11434/v1';
const models = (a.models || 'glm-5.3:cloud,kimi-k3:cloud').split(',');
const batchSize = Number(a.batch || 10);
const template = fs.readFileSync(new URL(`../../prompts/annotate_${gate}.txt`, import.meta.url), 'utf8');
const rows = readJsonl(path.join(a.pool, `${gate}.pool.jsonl`));
fs.mkdirSync(a.raw, { recursive: true });

for (const model of models) {
  const out = path.join(a.raw, `${gate}.${model.replace(/[^a-z0-9.-]/gi, '_')}.jsonl`);
  const done = new Set(readJsonl(out).map((r) => r.id));
  const todo = rows.filter((r) => !done.has(r.id)).sort((x, y) => unit(`${model}|${x.id}`) - unit(`${model}|${y.id}`));
  const batches = [];
  for (let i = 0; i < todo.length; i += batchSize) batches.push(todo.slice(i, i + batchSize));
  console.error(`${gate} ${model}: ${rows.length} rows, ${done.size} done, ${batches.length} batches`);
  let failed = 0;
  await pool(batches, Number(a.concurrency || 4), async (b, bi) => {
    const items = b.map((r, i) => `[i${i + 1}] ${spec.item(r)}`).join('\n\n');
    let pending = new Map(b.map((r, i) => [`i${i + 1}`, r]));
    for (let attempt = 0; attempt < 3 && pending.size; attempt++) {
      try {
        const res = await chat({ url, model, prompt: template.replace('{items}', items) });
        const labels = firstJson(res.text)?.labels;
        if (!Array.isArray(labels)) throw new Error('no labels array');
        for (const l of labels) {
          const r = pending.get(String(l.id));
          const parsed = r && spec.parse(l);
          if (!parsed) continue;
          fs.appendFileSync(out, JSON.stringify({ id: r.id, ...parsed, model: res.model, requested: model }) + '\n');
          pending.delete(String(l.id));
        }
        if (pending.size) throw new Error(`${pending.size} items missing or invalid`);
      } catch (e) {
        console.error(`${model} batch ${bi} attempt ${attempt}: ${e.message}`);
      }
    }
    failed += pending.size;
    if (bi % 10 === 0) console.error(`${model} batch ${bi}/${batches.length}`);
  });
  console.error(`${gate} ${model}: done, ${failed} items unlabelled`);
}
