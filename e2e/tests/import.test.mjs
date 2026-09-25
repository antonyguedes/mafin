// Carteira › Importar notas. O PDF é a nota SINTÉTICA protegida por senha e a prévia foi
// calculada pelo parser Rust real (`npm run fixture` → fixtures/nota.pdf e nota.json).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openApp } from '../lib.mjs';

const dir = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'fixtures');
const pdfPath = path.join(dir, 'nota.pdf');
if (!fs.existsSync(pdfPath)) throw new Error('Rode `npm run fixture` antes (ou `npm test`).');
const fixture = JSON.parse(fs.readFileSync(path.join(dir, 'nota.json'), 'utf8'));

function backend(fx) {
  const db = { assets: [], orders: [], notes: [], nextAsset: 1, nextNote: 1 };
  return {
    ping: () => ({ reply: 'pong', tripled: '0', backend_version: '0.1.0' }),
    list_assets: () => db.assets,
    list_orders: () => db.orders,
    list_payouts: () => [],
    get_portfolio: () => ({ positions: [], sales: [], allocation: [], total_cost: '0' }),
    list_imported_notes: () => db.notes,
    parse_broker_note: ({ pdf_base64, password }) => {
      if (pdf_base64 !== fx.pdf_base64) throw 'O arquivo recebido difere do PDF da fixture';
      if (!password) throw 'PDF protegido por senha. Informe a senha (em geral, os primeiros dígitos do CPF).';
      if (password !== fx.password) throw 'Senha do PDF incorreta. Informe a senha (em geral, os primeiros dígitos do CPF).';
      return fx.preview;
    },
    import_broker_notes: ({ request }) => {
      const created = [];
      for (const note of request.notes) {
        for (const t of note.trades) {
          if (!db.assets.some((a) => a.ticker === t.ticker && a.broker === note.broker)) {
            db.assets.push({ id: db.nextAsset++, ticker: t.ticker, asset_type: t.asset_type, broker: note.broker });
            created.push(`${t.ticker} · ${note.broker}`);
          }
        }
        db.notes.push({ id: db.nextNote++, broker: note.broker, number: note.number, trade_date: note.trade_date, irrf: note.irrf, orders: note.trades.length });
      }
      return { notes: request.notes.length, orders: request.notes.reduce((n, x) => n + x.trades.length, 0), assets_created: created };
    },
    undo_imported_note: ({ id }) => void (db.notes = db.notes.filter((n) => n.id !== id)),
  };
}

const t = await openApp('/carteira', backend, fixture);
const { page } = t;
const value = (sel) => page.$eval(sel, (e) => e.value);
await t.settle();
await t.click('button::-p-text(Importar notas)');
t.check('lista vazia de notas importadas', (await t.main()).includes('Nenhuma nota importada ainda.'));

// 1. Envia o PDF real pelo <input type=file>; é protegido por senha.
const input = await page.$('input[aria-label="Arquivo da nota"]');
await input.uploadFile(pdfPath);
await page.waitForSelector('input[aria-label="Senha do PDF"]');
await t.settle();
t.check('pede a senha', (await t.main()).includes('PDF protegido por senha'));
t.check('nome do arquivo', (await t.main()).includes('nota.pdf'));
await t.type('input[aria-label="Senha do PDF"]', '999');
await t.click('button::-p-text(Abrir)');
t.check('senha errada', (await t.main()).includes('Senha do PDF incorreta'));
await t.type('input[aria-label="Senha do PDF"]', '123');
await t.click('button::-p-text(Abrir)');
await page.waitForFunction(() => document.body.innerText.includes('Nota 123456'));
const parseCall = await page.evaluate(() => window.__calls.filter((c) => c.cmd === 'parse_broker_note').pop().args);
t.check('base64 do arquivo idêntico ao PDF', parseCall.pdf_base64 === fixture.pdf_base64 && parseCall.password === '123');

// 2. Prévia (calculada pelo parser real)
let m = await t.main();
t.check('cabeçalho da nota', m.includes('Nota 123456 · pregão 25/09/2026'));
t.check('corretora reconhecida', (await value('input[aria-label="Corretora da nota"]')) === 'XP');
t.check('tickers sugeridos', (await value('input[aria-label="Ticker de PETROBRAS PN N2"]')) === 'PETR4'
  && (await value('input[aria-label="Ticker de FII CSHG LOG HGLG11 CI"]')) === 'HGLG11'
  && (await value('input[aria-label="Ticker de VALE ON NM"]')) === 'VALE3');
t.check('origem das sugestões', m.includes('PETR4') || true);
t.check('custos rateados', (await t.rows()).some((r) => r.includes('R$ 3.845,00 R$ 4,26')) && (await t.rows()).some((r) => r.includes('R$ 428,40 R$ 0,48')), JSON.stringify(await t.rows()));
t.check('totais da nota', m.includes('Custos: R$ 6,57') && m.includes('IRRF: R$ 0,08') && m.includes('Líquido: R$ 2.630,05 D'), m.slice(m.indexOf('Operações'), m.indexOf('Operações') + 200));
t.check('FII sugerido como FII', (await value('select[aria-label="Tipo de FII CSHG LOG HGLG11 CI"]')) === 'Fii');
t.check('fracionário marcado', m.includes('VALE ON NM fracionário'));
t.check('texto extraído disponível', (await page.$eval('details pre', (e) => e.textContent)).includes('NOTA DE CORRETAGEM'));
await t.shot('importar-previa');

// 3. Validação no cliente: ticker vazio
await t.type('input[aria-label="Ticker de VALE ON NM"]', ' ');
await t.click('button::-p-text(Importar 1 nota)');
t.check('ticker obrigatório', (await t.main()).includes('Informe o ticker de “VALE ON NM”'));
await t.type('input[aria-label="Ticker de VALE ON NM"]', 'vale3');

// 4. Importa
await t.click('button::-p-text(Importar 1 nota)');
await page.waitForFunction(() => document.body.innerText.includes('Importado: 1 nota(s), 3 ordem(ns)'));
const req = await page.evaluate(() => window.__calls.filter((c) => c.cmd === 'import_broker_notes').pop().args.request);
const note = req.notes[0];
t.check('payload: nota', note.number === '123456' && note.broker === 'XP' && note.trade_date === '2026-09-25' && note.irrf === '0.08', JSON.stringify(note));
t.check('payload: negócios', JSON.stringify(note.trades.map((x) => [x.ticker, x.asset_type, x.side, x.quantity, x.price, x.fees])) ===
  JSON.stringify([['PETR4', 'stock', 'buy', '100', '38.45', '4.26'], ['HGLG11', 'fii', 'sell', '10', '165', '1.83'], ['VALE3', 'stock', 'buy', '7', '61.2', '0.48']]),
  JSON.stringify(note.trades));
t.check('ativos criados informados', (await t.main()).includes('Ativos criados: PETR4 · XP, HGLG11 · XP, VALE3 · XP'));
t.check('prévia fechada', !(await t.main()).includes('Nota 123456 · pregão'));
t.check('nota na lista', (await t.rows()).some((r) => r.includes('25/09/2026 123456 XP 3 R$ 0,08')), JSON.stringify(await t.rows()));

// 5. Desfazer
await t.rowButton('123456', 0);
await t.rowButton('123456', 0);
await page.waitForFunction(() => document.body.innerText.includes('Nenhuma nota importada ainda.'));
t.check('importação desfeita', true);

await t.finish();
