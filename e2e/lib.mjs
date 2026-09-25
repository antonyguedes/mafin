// Infra comum dos testes E2E: servidor estático do frontend/dist (com fallback SPA),
// Chrome headless via puppeteer-core e um mini "assert" com relatório PASS/FAIL.
//
// O backend Tauri é substituído por um `window.__TAURI__` falso injetado antes do app
// (cada teste define os comandos de que precisa). O IPC real, o SQLite e o motor Rust
// são cobertos pelos testes `cargo test`.

import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import puppeteer from 'puppeteer-core';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const DIST = path.join(ROOT, 'frontend', 'dist');
export const SHOTS = path.join(ROOT, 'e2e', 'shots');

export const MONTHS_PT = ['Janeiro', 'Fevereiro', 'Março', 'Abril', 'Maio', 'Junho', 'Julho', 'Agosto', 'Setembro', 'Outubro', 'Novembro', 'Dezembro'];
export const monthLabel = (year, month) => `${MONTHS_PT[month - 1]} de ${year}`;
export function currentMonth() {
  const now = new Date();
  return { year: now.getFullYear(), month: now.getMonth() + 1 };
}

const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };

function findChrome() {
  const candidates = [
    process.env.CHROME_PATH,
    '/usr/bin/google-chrome-stable',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser',
    '/usr/bin/brave',
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
  ];
  const found = candidates.find((p) => p && fs.existsSync(p));
  if (!found) throw new Error('Chrome/Chromium não encontrado. Defina CHROME_PATH.');
  return found;
}

/**
 * Sobe o servidor e o navegador, injeta o backend falso e abre `route`.
 * `fakeBackend` é serializado e executado no navegador: não pode usar variáveis do Node,
 * só o argumento `data`.
 */
export async function openApp(route, fakeBackend, data = null) {
  if (!fs.existsSync(path.join(DIST, 'index.html'))) {
    throw new Error('frontend/dist não existe. Rode `trunk build` em frontend/ (ou `npm test`, que faz isso).');
  }
  fs.mkdirSync(SHOTS, { recursive: true });

  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split('?')[0]);
    let file = path.join(DIST, url);
    if (!file.startsWith(DIST) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
      file = path.join(DIST, 'index.html');
    }
    res.writeHead(200, { 'content-type': MIME[path.extname(file)] ?? 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));

  const browser = await puppeteer.launch({
    executablePath: findChrome(),
    headless: true,
    args: ['--no-sandbox'],
    defaultViewport: { width: 1280, height: 1000 },
  });
  const page = await browser.newPage();
  const consoleErrors = [];
  page.on('console', (m) => m.type() === 'error' && consoleErrors.push(m.text()));
  page.on('pageerror', (e) => consoleErrors.push(String(e)));

  await page.evaluateOnNewDocument(installFakeTauri, fakeBackend.toString(), data);
  await page.goto(`http://127.0.0.1:${server.address().port}${route}`, { waitUntil: 'networkidle0' });

  let failures = 0;
  const t = {
    page,
    check(name, cond, detail = '') {
      console.log(`${cond ? 'PASS' : 'FAIL'}  ${name}${cond ? '' : `  -> ${detail}`}`);
      if (!cond) failures++;
    },
    settle: (ms = 300) => new Promise((r) => setTimeout(r, ms)),
    async click(selector) {
      await page.locator(selector).click();
      await t.settle();
    },
    /** Limpa o campo e digita (dispara os eventos `input` que o `bind:value` escuta). */
    async type(selector, value) {
      await page.$eval(selector, (el) => {
        el.value = '';
        el.dispatchEvent(new Event('input', { bubbles: true }));
      });
      await page.type(selector, value);
    },
    /** Define o valor direto (date, select, campos sem placeholder). */
    async setValue(selector, value) {
      await page.$eval(
        selector,
        (el, v) => {
          el.value = v;
          el.dispatchEvent(new Event('input', { bubbles: true }));
          el.dispatchEvent(new Event('change', { bubbles: true }));
        },
        value,
      );
    },
    async selectByText(selector, text) {
      const value = await page.$eval(selector, (el, text) => [...el.options].find((o) => o.text === text)?.value, text);
      if (value === undefined) throw new Error(`opção "${text}" não encontrada em ${selector}`);
      await t.setValue(selector, value);
    },
    main: () => page.$eval('main', (m) => m.innerText.replace(/\s+/g, ' ')),
    rows: (scope = 'body') =>
      page.$$eval(`${scope} tbody tr`, (trs) => trs.map((tr) => tr.innerText.replace(/\s+/g, ' ').trim())),
    /** Clica no n-ésimo botão da linha cujo texto contém `match`. */
    async rowButton(match, index) {
      const texts = await t.rows();
      const i = texts.findIndex((r) => r.includes(match));
      if (i < 0) throw new Error(`linha com "${match}" não encontrada: ${JSON.stringify(texts)}`);
      const buttons = await (await page.$$('tbody tr'))[i].$$('button');
      await buttons[index].click();
      await t.settle();
    },
    /** O gráfico com esse aria-label foi desenhado (canvas do ECharts presente)? */
    hasChart: (label) =>
      page.waitForFunction((l) => document.querySelector(`[aria-label="${l}"] canvas`), { timeout: 5000 }, label).then(
        () => true,
        () => false,
      ),
    monthLabel: () => page.$eval('header span.min-w-44', (e) => e.innerText),
    /** Navega o seletor de mês (‹ ›) até `year`/`month`, independente da data de hoje. */
    async gotoMonth(year, month) {
      for (let i = 0; i < 600; i++) {
        const [name, , y] = (await t.monthLabel()).split(' ');
        const diff = year * 12 + month - (Number(y) * 12 + MONTHS_PT.indexOf(name) + 1);
        if (diff === 0) return t.settle();
        await page.click(diff < 0 ? 'button[aria-label="Mês anterior"]' : 'button[aria-label="Próximo mês"]');
        await t.settle(40);
      }
      throw new Error(`não chegou em ${month}/${year}`);
    },
    shot: (name, fullPage = true) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), fullPage }),
    async finish() {
      t.check('sem erros no console', consoleErrors.length === 0, consoleErrors.join(' | '));
      await browser.close();
      server.close();
      console.log(failures ? `\n${failures} falha(s)` : '\nTodos os checks passaram');
      process.exitCode = failures ? 1 : 0;
    },
  };
  return t;
}

// Executado no navegador. `handlersSource` é o código de uma função `(data) => ({ cmd: fn })`.
function installFakeTauri(handlersSource, data) {
  // eslint-disable-next-line no-new-func
  const handlers = new Function(`return (${handlersSource})`)()(data);
  const clone = (x) => (x === undefined ? null : JSON.parse(JSON.stringify(x)));
  window.__calls = [];
  window.__TAURI__ = {
    core: {
      async invoke(cmd, args) {
        window.__calls.push({ cmd, args: clone(args ?? {}) });
        await new Promise((r) => setTimeout(r, 15));
        if (!handlers[cmd]) throw `comando desconhecido: ${cmd}`;
        return clone(await handlers[cmd](args ?? {}));
      },
    },
  };
}
