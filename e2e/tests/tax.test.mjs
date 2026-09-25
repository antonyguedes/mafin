// Tela de IR. O relatório vem do motor Rust REAL: fixtures/tax.json é gerado por
// `cargo run -p shared --example tax_fixture` (ver package.json, script "fixture").
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openApp } from '../lib.mjs';

const fixturePath = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'fixtures', 'tax.json');
if (!fs.existsSync(fixturePath)) throw new Error('Rode `npm run fixture` antes (ou `npm test`).');
const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));

function backend(fx) {
  const irrf = {};
  return {
    ping: () => ({ reply: 'pong', tripled: '0', backend_version: '0.1.0' }),
    list_transactions: () => [],
    get_portfolio: () => fx.portfolio,
    get_tax_report: () => fx.tax,
    set_monthly_irrf: ({ month, amount }) => { irrf[`${month.year}-${month.month}`] = amount; },
  };
}

const t = await openApp('/imposto-de-renda', backend, fixture);
const { page } = t;
const cards = () => page.$$eval('main > div > div.grid > section', (s) => s.map((x) => x.innerText.replace(/\s+/g, ' ').trim()));
await page.waitForFunction(() => document.body.innerText.includes('Resumo de'));
await t.gotoMonth(2026, 9);

// Setembro: VALE3 comprada e vendida no mesmo dia vira day trade (20%); FII 20%.
let c = await cards();
t.check('4 cards', c.length === 4, JSON.stringify(c));
t.check('set: day trade fora do limite de 20 mil', c[0].includes('R$ 0,00') && c[0].includes('Isento'), c[0]);
t.check('set: lucro tributável por categoria', c[1].includes('R$ 225,00') && c[1].includes('Ações (day trade) R$ 200,00 · FIIs R$ 25,00'), c[1]);
t.check('set: DARF 45 vence 30/10', c[3].includes('R$ 45,00') && c[3].includes('30/10/2026'), c[3]);
let m = await t.main();
t.check('set: aviso de day trade', m.includes('Day trade no mês: VALE3'));
t.check('set: coluna de day trade a 20%', m.toLowerCase().includes('ações ações (day trade) fiis') && m.includes('Imposto R$ 0,00 R$ 40,00 R$ 5,00'), m.slice(m.indexOf('Apuração'), m.indexOf('Apuração') + 700));
t.check('set: venda marcada day trade', m.includes('VALE3 day trade'));
t.check('set: estimativa de IRRF', m.includes('Estimativa pelas suas ordens: R$ 2,04'));

// Informar IRRF grava o valor do mês
await t.type('input[aria-label="IRRF do mês"]', '2,04');
await t.click('button::-p-text(Salvar IRRF)');
const saved = await page.evaluate(() => window.__calls.filter((c) => c.cmd === 'set_monthly_irrf').pop()?.args);
t.check('IRRF enviado como decimal', saved?.amount === '2.04' && saved?.month.year === 2026 && saved?.month.month === 9, JSON.stringify(saved));
t.check('confirmação de salvo', (await t.main()).includes('Salvo ✓'));
await t.shot('ir-setembro');

// Agosto: tributável, compensa prejuízo de julho, DARF 387
await t.gotoMonth(2026, 8);
c = await cards();
t.check('ago: acima do limite', c[0].includes('R$ 21.600,00') && c[0].includes('Acima de R$ 20.000,00'), c[0]);
t.check('ago: base = 2.380 + 150', c[1].includes('R$ 2.530,00'), c[1]);
t.check('ago: DARF = 387 − 1,49 de IRRF', c[3].includes('R$ 385,51') && c[3].includes('30/09/2026'), c[3]);
m = await t.main();
t.check('ago: IRRF carregado no campo', (await page.$eval('input[aria-label="IRRF do mês"]', (e) => e.value)) === '1,49');
t.check('ago: IRRF abatido no rodapé', m.includes('(−) IRRF abatido −R$ 1,49'));
t.check('ago: compensação', m.includes('Prejuízo compensado R$ 1.209,00'), m.slice(m.indexOf('Apuração'), m.indexOf('Apuração') + 600));
t.check('ago: impostos por categoria', m.includes('R$ 357,00') && m.includes('R$ 30,00'));
await t.shot('ir-agosto');

// Julho: prejuízo em mês isento fica acumulado
await t.gotoMonth(2026, 7);
c = await cards();
t.check('jul: prejuízo acumulado', c[0].includes('R$ 10.800,00') && c[2].includes('R$ 1.209,00'), JSON.stringify(c));

// Junho: sem vendas
await t.gotoMonth(2026, 6);
t.check('jun: sem vendas', (await t.main()).includes('Sem vendas de ações ou FIIs em Junho de 2026'));

// Resumo anual: clicar numa linha seleciona o mês
const yearRows = await page.evaluate(() => {
  const sec = [...document.querySelectorAll('section')].find((s) => s.querySelector('h2')?.innerText.startsWith('Resumo de'));
  return [...sec.querySelectorAll('tbody tr')].map((r) => r.innerText.replace(/\s+/g, ' ').trim());
});
t.check('resumo anual com 3 meses', yearRows.length === 3 && yearRows[1].startsWith('Agosto') && yearRows[1].includes('R$ 385,51'), JSON.stringify(yearRows));
await page.evaluate(() => [...document.querySelectorAll('tbody tr')].find((r) => r.innerText.startsWith('Agosto')).click());
await t.settle();
t.check('total de DARFs no ano', (await t.main()).includes('DARFs no ano: R$ 430,51'));
t.check('clique no resumo seleciona agosto', (await t.monthLabel()) === 'Agosto de 2026');

await t.finish();
