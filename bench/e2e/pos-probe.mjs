// Controlled probe: does an identical DOM mutation cost more when the <ul> it
// happens in sits EARLIER in the combo container (= has more following <li>s),
// exactly as loom-bench mounts K,C,D,E,D-LIS,E-LIS into one container?
//
// Everything except document position is identical: same code, same n, same
// row shape (<li> + one text node), same page, position order reshuffled every
// rep. Served with COOP/COEP so performance.now() has the same 5us resolution
// the real harness runs under.
import http from 'node:http';
import fs from 'node:fs';
import { chromium, firefox } from '@playwright/test';

const N = Number(process.env.N ?? 10000);
const REPS = Number(process.env.REPS ?? 60);
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
  const resolution = (() => {
    let min = Infinity;
    for (let i = 0; i < 32; i++) {
      const a = performance.now();
      let b = performance.now(), spins = 0;
      while (b === a && spins < 1e6) { b = performance.now(); spins++; }
      if (b - a > 0 && b - a < min) min = b - a;
    }
    return min;
  })();

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

  let s = 12345;
  const rnd = () => (s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff;

  const K = 10; // harness batch size
  const W = {
    value_update: { batch: 1, fwd: (ul, p, j) => {
      const kids = ul.childNodes;
      for (let i = 0; i < kids.length; i++) kids[i].firstChild.data = 'v' + p + j + '-' + i;
    } },
    head_insert: { batch: K, fwd: (ul, p, j) => {
      if (j % 2 === 0) {
        const li = document.createElement('li');
        li.appendChild(document.createTextNode('new' + p));
        ul.insertBefore(li, ul.firstChild);
      } else { ul.removeChild(ul.firstChild); }
    } },
    swap: { batch: K, fwd: (ul) => {
      const kids = ul.children;
      const a = kids[1], b = kids[kids.length - 2];
      const aNext = a.nextSibling, bNext = b.nextSibling;
      if (ul.moveBefore) { ul.moveBefore(a, bNext); ul.moveBefore(b, aNext); }
      else { ul.insertBefore(a, bNext); ul.insertBefore(b, aNext); }
    } },
    respawn: { batch: 1, reps: 12, fwd: (ul, p, j) => {
      ul.textContent = null;
      for (let i = 0; i < N; i++) {
        const li = document.createElement('li');
        li.appendChild(document.createTextNode('r' + p + j + '-' + i));
        ul.appendChild(li);
      }
    } },
  };

  const res = {};
  for (const [name, w] of Object.entries(W)) {
    const reps = w.reps ?? REPS;
    const sync = Array.from({ length: 6 }, () => []);
    const frame = Array.from({ length: 6 }, () => []);
    for (let rep = 0; rep < reps; rep++) {
      const order = [0, 1, 2, 3, 4, 5];
      for (let i = 5; i > 0; i--) { const j = Math.floor(rnd() * (i + 1)); [order[i], order[j]] = [order[j], order[i]]; }
      for (const p of order) {
        await raf();
        const t0 = performance.now();
        for (let j = 0; j < w.batch; j++) w.fwd(uls[p], p, j);
        const t1 = performance.now();
        sync[p].push((t1 - t0) / w.batch);
        await raf();
        frame[p].push(performance.now() - t0);
      }
    }
    res[name] = { sync, frame };
  }
  const following = uls.map((_, i) => uls.slice(i + 1).reduce((a, u) => a + u.childElementCount, 0));
  return { res, following, resolution, isolated: self.crossOriginIsolated === true, n: N };
}, { N, REPS });

const med = (a) => { const b = [...a].sort((x, y) => x - y); return b.length % 2 ? b[(b.length - 1) / 2] : (b[b.length / 2 - 1] + b[b.length / 2]) / 2; };
const pairedCI = (a, b, iters = 10000) => {
  const n = Math.min(a.length, b.length);
  const d = [];
  for (let i = 0; i < n; i++) if (a[i] > 0 && b[i] > 0) d.push(Math.log(a[i] / b[i]));
  if (!d.length) return [NaN, NaN];
  const rs = [];
  for (let it = 0; it < iters; it++) {
    let sum = 0;
    for (let i = 0; i < d.length; i++) sum += d[Math.floor(Math.random() * d.length)];
    rs.push(Math.exp(sum / d.length));
  }
  rs.sort((x, y) => x - y);
  return [rs[Math.floor(iters * 0.005)], rs[Math.floor(iters * 0.995)]];
};

for (const [name, launcher] of [['chromium', chromium], ['firefox', firefox]]) {
  const browser = await launcher.launch();
  const page = await browser.newPage();
  await page.goto(url);
  const out = await probe(page);
  console.log(`===== ${name} n=${out.n} isolated=${out.isolated} resolution=${out.resolution.toFixed(4)}ms following-li/pos=${out.following.join(',')}`);
  for (const [w, v] of Object.entries(out.res)) {
    const meds = v.sync.map(med);
    const fmeds = v.frame.map(med);
    const ci = pairedCI(v.sync[0], v.sync[5]);
    console.log(`  ${w.padEnd(13)} sync median/op by pos  : ${meds.map((x) => x.toFixed(5)).join('  ')}`);
    console.log(`  ${''.padEnd(13)} ratio vs pos5          : ${meds.map((x) => (x / meds[5]).toFixed(3)).join('  ')}`);
    console.log(`  ${''.padEnd(13)} paired 99% CI pos0/pos5: [${ci[0].toFixed(3)}, ${ci[1].toFixed(3)}]`);
    console.log(`  ${''.padEnd(13)} frame median by pos    : ${fmeds.map((x) => x.toFixed(2)).join('  ')}`);
  }
  await browser.close();
}
server.close();
