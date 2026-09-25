// Carteira › Proventos: cadastro, sugestão de IR do JCP, totais, yield on cost, IRPF.
import { openApp } from '../lib.mjs';

function backend() {
  const assets = [
    { id: 1, ticker: 'HGLG11', asset_type: 'fii', broker: 'XP' },
    { id: 2, ticker: 'PETR4', asset_type: 'stock', broker: 'XP' },
  ];
  const allowed = { stock: ['dividend', 'jcp', 'other'], fii: ['fii_income', 'other'], fixed_income: ['other'] };
  const db = { nextId: 1, payouts: [] };
  const check = (input) => {
    const asset = assets.find((a) => a.id === input.asset_id);
    if (!allowed[asset.asset_type].includes(input.kind)) throw `Tipo não se aplica a ${asset.ticker}`;
  };
  const position = (ticker, asset_type, quantity, average_price, total_cost) =>
    ({ ticker, asset_type, brokers: ['XP'], quantity, average_price, total_cost });
  return {
    ping: () => ({ reply: 'pong', tripled: '0', backend_version: '0.1.0' }),
    list_transactions: () => [],
    get_tax_report: () => ({ months: [], losses: { stock: '0', day_trade: '0', fii: '0' }, pending_below_minimum: '0', irrf_credit: '0' }),
    list_assets: () => assets,
    list_orders: () => [],
    get_portfolio: () => ({
      positions: [position('HGLG11', 'fii', '10', '160', '1600'), position('PETR4', 'stock', '100', '40', '4000')],
      sales: [],
      allocation: [
        { asset_type: 'stock', cost: '4000', percent: '71.43' },
        { asset_type: 'fii', cost: '1600', percent: '28.57' },
      ],
      total_cost: '5600',
    }),
    list_payouts: () => [...db.payouts].sort((a, b) => (a.date === b.date ? b.id - a.id : a.date < b.date ? 1 : -1)),
    create_payout: ({ input }) => {
      check(input);
      const p = { id: db.nextId++, ...input };
      db.payouts.push(p);
      return p;
    },
    update_payout: ({ id, input }) => {
      check(input);
      const i = db.payouts.findIndex((p) => p.id === id);
      return (db.payouts[i] = { id, ...input });
    },
    delete_payout: ({ id }) => void (db.payouts = db.payouts.filter((p) => p.id !== id)),
  };
}

const t = await openApp('/carteira', backend);
const { page } = t;
const text = async (sel) => page.$eval(sel, (e) => e.innerText.replace(/\s+/g, ' ').trim());
const kindOptions = () => page.$$eval('form div:has(> span) button[aria-pressed]', (bs) => bs.map((b) => b.innerText));
const lastCall = (cmd) => page.evaluate((cmd) => window.__calls.filter((c) => c.cmd === cmd).pop()?.args, cmd);
async function gotoYear(year) {
  for (let i = 0; i < 50; i++) {
    const current = Number(await text('[data-testid=payout-year]'));
    if (current === year) return t.settle();
    await page.click(current > year ? 'button[aria-label="Ano anterior"]' : 'button[aria-label="Próximo ano"]');
    await t.settle(60);
  }
}
const add = async ({ asset, kind, date, gross, withheld }) => {
  await t.selectByText('form select', asset);
  await t.settle(150);
  if (kind) await t.click(`form button::-p-text(${kind})`);
  await t.setValue('input[type=date]', date);
  await t.type('input[aria-label="Valor bruto"]', gross);
  if (withheld !== undefined) await t.type('input[aria-label="IR retido"]', withheld);
  await t.settle(150);
};

await t.settle();
await t.click('button::-p-text(Proventos)');
await gotoYear(2026);
t.check('estado vazio', (await t.main()).includes('Nenhum provento em 2026.'));

// Ação: tipos de ação; JCP sugere 15% de IR
await t.selectByText('form select', 'PETR4 · XP');
await t.settle(200);
t.check('tipos para ação', JSON.stringify(await kindOptions()) === JSON.stringify(['Dividendos', 'JCP', 'Outros']), JSON.stringify(await kindOptions()));
await add({ asset: 'PETR4 · XP', kind: 'JCP', date: '2026-09-10', gross: '123,45' });
t.check('JCP sugere 15% (18,5175 → 18,52)', (await page.$eval('input[aria-label="IR retido"]', (e) => e.value)) === '18,52');
t.check('prévia do líquido', (await text('form')).includes('Líquido: R$ 104,93'));
await t.click('button::-p-text(Registrar provento)');
await page.waitForFunction(() => document.querySelectorAll('tbody tr').length >= 1);
const jcp = await lastCall('create_payout');
t.check('payload do JCP', jcp.input.kind === 'jcp' && jcp.input.gross === '123.45' && jcp.input.withheld === '18.52', JSON.stringify(jcp));

await add({ asset: 'PETR4 · XP', kind: 'Dividendos', date: '2026-03-15', gross: '200' });
t.check('dividendo sem IR sugerido', (await page.$eval('input[aria-label="IR retido"]', (e) => e.value)) === '');
await t.click('button::-p-text(Registrar provento)');

// FII: tipo muda sozinho para Rendimentos
await t.selectByText('form select', 'HGLG11 · XP');
await t.settle(200);
t.check('tipos para FII', JSON.stringify(await kindOptions()) === JSON.stringify(['Rendimentos', 'Outros']), JSON.stringify(await kindOptions()));
await add({ asset: 'HGLG11 · XP', date: '2026-09-15', gross: '80,50' });
await t.click('button::-p-text(Registrar provento)');
await page.waitForFunction(() => document.body.innerText.includes('R$ 385,43'));

// Totais, gráfico, por ativo e IRPF
let m = await t.main();
t.check('líquido no ano', m.includes('Líquido no ano R$ 385,43') && m.includes('IR retido R$ 18,52'), m.slice(0, 600));
t.check('cards por tipo', m.includes('Dividendos R$ 200,00') && m.includes('JCP (líquido) R$ 104,93') && m.includes('Rendimentos de FII R$ 80,50'));
t.check('gráfico mensal', await t.hasChart('Proventos líquidos por mês'));
t.check('yield on cost', m.includes('PETR4 R$ 304,93 7,62%') && m.includes('HGLG11 R$ 80,50 5,03%'), m.slice(m.indexOf('Por ativo'), m.indexOf('Por ativo') + 200));
t.check('IRPF: título e fichas', m.includes('Para a declaração de 2027 (ano-calendário 2026)') && m.includes('código 09') && m.includes('código 10'));
t.check('IRPF: JCP líquido com bruto e IR', m.includes('R$ 104,93 (bruto R$ 123,45 − IR R$ 18,52)'));
await t.shot('proventos');

// Editar: IR retido digitado à mão não é sobrescrito pela sugestão
await t.rowButton('JCP', 0);
t.check('provento em edição', (await text('form h2')) === 'Editando provento');
await t.type('input[aria-label="IR retido"]', '20');
await t.type('input[aria-label="Valor bruto"]', '130');
t.check('IR manual preservado', (await page.$eval('input[aria-label="IR retido"]', (e) => e.value)) === '20');
await t.click('button::-p-text(Salvar alterações)');
await page.waitForFunction(() => document.body.innerText.includes('R$ 390,50')); // 110 + 200 + 80,50

// Excluir o rendimento do FII
await t.rowButton('HGLG11', 1);
await t.rowButton('HGLG11', 1);
await page.waitForFunction(() => document.body.innerText.replace(/\s+/g, ' ').includes('Líquido no ano R$ 310,00'));
t.check('excluído', !(await t.rows()).some((r) => r.includes('HGLG11')));

// Alerta de dividendos acima de R$ 50 mil de uma mesma empresa no mês
await add({ asset: 'PETR4 · XP', kind: 'Dividendos', date: '2026-05-05', gross: '60.000,00' });
await t.click('button::-p-text(Registrar provento)');
await page.waitForFunction(() => document.body.innerText.includes('Lei 15.270/2025'));
t.check('alerta de dividendos > 50 mil', (await t.main()).includes('PETR: R$ 60.000,00 em dividendos em Maio de 2026'));

// Outro ano
await gotoYear(2025);
t.check('2025 vazio', (await t.main()).includes('Nenhum provento em 2025.'));

// Dashboard mostra o card de proventos, sem erros de carga
await t.click('a[href="/"]');
await page.waitForFunction(() => document.body.innerText.includes('Proventos do mês'));
await t.settle();
t.check('dashboard: card de proventos', /Proventos do mês R\$ [\d.,]+/.test(await t.main()));
t.check('dashboard sem erros', (await page.$$('[role=alert]')).length === 0);

await t.finish();
