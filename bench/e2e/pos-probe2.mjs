// Decomposition probe. Phase A: 6 identical lists, document order = creation
// order (exactly loom-bench's layout). Phase B: the SAME list objects, document
// order reversed. If the timed bracket (M1(a) = synchronous tick) carries a
// document-position cost, medians grouped by document position stay put across
// phases while medians grouped by list identity flip. If it carries none, the
// pattern is noise and neither grouping is stable.
import http from 'node:http';
import fs from 'node:fs';
import { chromium, firefox } from '@playwright/test';

const N = Number(process.env.N ?? 10000);
const REPS = Number(process.env.REPS ?? 100);
const HTML = fs.readFileSync(process.argv[2], 'utf8');

const server = http.createServer((req, res) => {
  res.writeHead(200, {
    'Content-Type': 'text/html; charset=utf-8',
    'Cross-Origin-Opener-Policy': 'same-origin',
    'Cross-Origin-Embedder-Policy': 'require-corp',
  });
  res.end(HTML);
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const url = `http://127.0.0.1:${server.address().port}/`;

const probe = (page) => page.evaluate(async ({ N, REPS }) => {
  const raf = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  const bench = document.getElementById('bench');
  const combo = document.createElement('div');
  bench.appendChild(combo);
  const uls = [];
  for (let p = 0; p < 6; p++) {
    const h = document.createElement('h4'); h.textContent = 'P' + p; combo.appendChild(h);
    const ul = document.createElement('ul'); combo.appendChild(ul); uls.push(ul);
    for (let i = 0; i < N; i++) {
      const li = document.createElement('li');
      li.appendChild(document.createTextNode('item-' + i));
      ul.appendChild(li);
    }
  }
  await raf();

  let s = 987654321;
  const rnd = () => (s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff;

  const valueUpdate = (ul, tag) => {
    const kids = ul.childNodes;
    for (let i = 0; i < kids.length; i++) kids[i].firstChild.data = 'v' + tag + '-' + i;
  };
  const respawn = (ul, tag) => {
    ul.textContent = null;
    for (let i = 0; i < N; i++) {
      const li = document.createElement('li');
      li.appendChild(document.createTextNode('r' + tag + '-' + i));
      ul.appendChild(li);
    }
  };

  const measure = async (fn, reps) => {
    const byList = Array.from({ length: 6 }, () => []);
    for (let rep = 0; rep < reps; rep++) {
      const order = [0, 1, 2, 3, 4, 5];
      for (let i = 5; i > 0; i--) { const j = Math.floor(rnd() * (i + 1)); [order[i], order[j]] = [order[j], order[i]]; }
      for (const li of order) {
        await raf();
        const t0 = performance.now();
        fn(uls[li], li);
        byList[li].push(performance.now() - t0);
        await raf();
      }
    }
    return byList;
  };

  const docPositions = () => uls.map((ul) => Array.prototype.indexOf.call(combo.querySelectorAll('ul'), ul));

  const A = { value_update: await measure(valueUpdate, REPS), respawn: await measure(respawn, 15), pos: docPositions() };

  // reverse document order of the SAME <ul> objects (headings follow along)
  for (let i = 5; i >= 0; i--) {
    combo.appendChild(uls[i].previousSibling); // its <h4>
    combo.appendChild(uls[i]);
  }
  await raf();

  const B = { value_update: await measure(valueUpdate, REPS), respawn: await measure(respawn, 15), pos: docPositions() };
  return { A, B, n: N, isolated: self.crossOriginIsolated === true };
}, { N, REPS });

const med = (a) => { const b = [...a].sort((x, y) => x - y); return b.length % 2 ? b[(b.length - 1) / 2] : (b[b.length / 2 - 1] + b[b.length / 2]) / 2; };

for (const [name, launcher] of [['chromium', chromium], ['firefox', firefox]]) {
  const browser = await launcher.launch();
  const page = await browser.newPage();
  await page.goto(url);
  const { A, B, n, isolated } = await probe(page);
  console.log(`===== ${name} n=${n} isolated=${isolated}`);
  console.log(`  phase A doc positions of lists 0..5: ${A.pos.join(',')}   phase B: ${B.pos.join(',')}`);
  for (const w of ['value_update', 'respawn']) {
    const ma = A[w].map(med), mb = B[w].map(med);
    // group by document position
    const byPosA = A.pos.map((_, p) => ma[A.pos.indexOf(p)]);
    const byPosB = B.pos.map((_, p) => mb[B.pos.indexOf(p)]);
    console.log(`  ${w}`);
    console.log(`    by LIST identity  A: ${ma.map((x) => x.toFixed(3)).join('  ')}`);
    console.log(`    by LIST identity  B: ${mb.map((x) => x.toFixed(3)).join('  ')}`);
    console.log(`    by DOC position   A: ${byPosA.map((x) => x.toFixed(3)).join('  ')}  (pos0 has most following nodes)`);
    console.log(`    by DOC position   B: ${byPosB.map((x) => x.toFixed(3)).join('  ')}`);
    console.log(`    pos0/pos5 ratio   A: ${(byPosA[0] / byPosA[5]).toFixed(3)}   B: ${(byPosB[0] / byPosB[5]).toFixed(3)}`);
  }
  await browser.close();
}
server.close();
