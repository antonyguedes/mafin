// Carteira (ativos, ordens, custódia) + Dashboard.
// O backend falso reimplementa as regras do motor em JS só para dirigir a UI; a exatidão
// dos cálculos é garantida pelos testes Rust (shared::portfolio / src-tauri ledger).
import { openApp } from '../lib.mjs';

function backend() {
  let db = { nextAsset: 1, nextOrder: 1, assets: [], orders: [] };
  const clone = (x) => JSON.parse(JSON.stringify(x));
  const str = (n) => String(Math.round(n * 1e10) / 1e10);
  const fmtDate = (d) => d.split('-').reverse().join('/');

  function build(state) {
    const types = {};
    for (const a of state.assets) types[a.ticker] = a.asset_type;
    const byId = Object.fromEntries(state.assets.map((a) => [a.id, a]));
    const books = {};
    const sales = [];
    const sorted = [...state.orders].sort((a, b) => (a.date === b.date ? a.id - b.id : a.date < b.date ? -1 : 1));
    for (const o of sorted) {
      const a = byId[o.asset_id];
      const b = (books[a.ticker] ??= { qty: 0, cost: 0, brokers: new Set() });
      b.brokers.add(a.broker);
      const [q, p, f] = [Number(o.quantity), Number(o.price), Number(o.fees)];
      if (o.kind === 'buy') {
        b.cost += q * p + f;
        b.qty += q;
        continue;
      }
      if (q > b.qty) throw `Venda de ${q} ${a.ticker} em ${fmtDate(o.date)} excede a posição de ${b.qty}`;
      const cost = q === b.qty ? b.cost : (b.cost * q) / b.qty;
      sales.push({
        order_id: o.id, ticker: a.ticker, asset_type: a.asset_type, date: o.date, quantity: o.quantity, kind: 'swing', day_trade: false,
        gross: str(q * p), fees: o.fees, cost: str(cost), result: str(q * p - f - cost),
      });
      b.cost -= cost;
      b.qty -= q;
      if (b.qty === 0) b.cost = 0;
    }
    const positions = Object.entries(books)
      .filter(([, b]) => b.qty > 0)
      .sort(([x], [y]) => (x < y ? -1 : 1))
      .map(([ticker, b]) => ({
        ticker, asset_type: types[ticker], brokers: [...b.brokers].sort(), quantity: str(b.qty),
        average_price: str(b.cost / b.qty), total_cost: str(b.cost),
      }));
    const total = positions.reduce((s, p) => s + Number(p.total_cost), 0);
    const allocation = ['stock', 'fii', 'fixed_income']
      .map((t) => ({ t, c: positions.filter((p) => p.asset_type === t).reduce((s, p) => s + Number(p.total_cost), 0) }))
      .filter(({ c }) => c > 0)
      .map(({ t, c }) => ({ asset_type: t, cost: str(c), percent: str((c / total) * 100) }));
    return { positions, sales, allocation, total_cost: str(total) };
  }

  // Escrita "transacional": aplica numa cópia, valida, só então confirma.
  function checked(mutate) {
    const draft = clone(db);
    const result = mutate(draft);
    build(draft);
    db = draft;
    return result;
  }

  return {
    ping: () => ({ reply: 'pong', tripled: '0', backend_version: '0.1.0' }),
    list_transactions: () => [],
    list_categories: () => [],
    get_tax_report: () => ({ months: [], losses: { stock: '0', day_trade: '0', fii: '0' }, pending_below_minimum: '0', irrf_credit: '0' }),
    list_assets: () => [...db.assets].sort((a, b) => (a.ticker === b.ticker ? (a.broker < b.broker ? -1 : 1) : a.ticker < b.ticker ? -1 : 1)),
    create_asset: ({ input }) =>
      checked((d) => {
        const ticker = input.ticker.trim().toUpperCase();
        if (d.assets.some((a) => a.ticker === ticker && a.broker === input.broker.trim())) throw 'Este ticker já está cadastrado nesta corretora';
        const a = { id: d.nextAsset++, ticker, asset_type: input.asset_type, broker: input.broker.trim() };
        d.assets.push(a);
        return a;
      }),
    update_asset: ({ id, input }) => checked((d) => Object.assign(d.assets.find((x) => x.id === id), input)),
    delete_asset: ({ id }) => {
      if (db.orders.some((o) => o.asset_id === id)) throw 'O ativo possui ordens registradas; exclua as ordens antes';
      db.assets = db.assets.filter((a) => a.id !== id);
    },
    list_orders: () => [...db.orders].sort((a, b) => (a.date === b.date ? b.id - a.id : a.date < b.date ? 1 : -1)),
    create_order: ({ input }) => checked((d) => { const o = { id: d.nextOrder++, ...input }; d.orders.push(o); return o; }),
    update_order: ({ id, input }) => checked((d) => { const i = d.orders.findIndex((o) => o.id === id); return (d.orders[i] = { id, ...input }); }),
    delete_order: ({ id }) => checked((d) => { d.orders = d.orders.filter((o) => o.id !== id); }),
    get_portfolio: () => build(db),
    list_payouts: () => [],
  };
}

const t = await openApp('/carteira', backend);
const { page } = t;
const formText = () => page.$eval('form', (f) => f.innerText);
const order = async ({ asset, kind, qty, price, fees = '0', date }) => {
  await t.selectByText('form select', asset);
  await t.click(`form button::-p-text(${kind})`);
  await t.type('input[placeholder="100"]', qty);
  await t.type('input[placeholder="0,00"]', price);
  await t.setValue('input[aria-label="Taxas"]', fees);
  await t.setValue('input[type=date]', date);
  await t.settle(100);
};
const waitRows = (n) => page.waitForFunction((n) => document.querySelectorAll('tbody tr').length === n, {}, n);
await t.settle();

// 1. Carteira vazia leva ao cadastro
t.check('estado vazio', (await t.main()).includes('Nenhuma posição em carteira.'));
await t.click('button::-p-text(Cadastre seu primeiro ativo)');
t.check('foi para Ativos', (await t.main()).includes('Novo ativo'));

// 2. Ativos
await t.type('input[placeholder="Ex.: PETR4"]', ' petr4 ');
await t.type('input[list="brokers"]', 'XP');
await t.click('button::-p-text(Cadastrar ativo)');
await waitRows(1);
t.check('ativo normalizado', (await t.rows())[0].startsWith('PETR4 Ação XP'), JSON.stringify(await t.rows()));
t.check('corretora mantida', (await page.$eval('input[list="brokers"]', (e) => e.value)) === 'XP');
await t.type('input[placeholder="Ex.: PETR4"]', 'HGLG11');
await t.click('form button::-p-text(FII)');
await t.click('button::-p-text(Cadastrar ativo)');
await waitRows(2);
await t.type('input[placeholder="Ex.: PETR4"]', 'PETR4');
await t.click('form button::-p-text(Ação)');
await t.click('button::-p-text(Cadastrar ativo)');
t.check('duplicado rejeitado', (await t.main()).includes('Este ticker já está cadastrado nesta corretora'));
await t.type('input[placeholder="Ex.: PETR4"]', 'PETR 4');
await t.click('button::-p-text(Cadastrar ativo)');
t.check('validação de ticker no cliente', (await t.main()).includes('Use apenas letras e números'));

// 3. Ordens
await t.click('button::-p-text(Ordens)');
await order({ asset: 'PETR4 · XP', kind: 'Compra', qty: '100', price: '10', fees: '10', date: '2026-09-01' });
t.check('prévia do custo', (await formText()).includes('Custo total: R$ 1.010,00'));
await t.click('button::-p-text(Registrar ordem)');
await waitRows(1);
await order({ asset: 'PETR4 · XP', kind: 'Compra', qty: '100', price: '12', fees: '10', date: '2026-09-02' });
await t.click('button::-p-text(Registrar ordem)');
await waitRows(2);
await order({ asset: 'PETR4 · XP', kind: 'Venda', qty: '50', price: '15', fees: '5', date: '2026-09-03' });
t.check('prévia da venda', (await formText()).includes('Valor líquido: R$ 745,00'));
await t.click('button::-p-text(Registrar ordem)');
await waitRows(3);
await t.settle();
const sale = (await t.rows()).find((r) => r.includes('Venda'));
t.check('resultado da venda = 750 − 5 − 50×11,10', sale?.includes('+R$ 190,00'), sale);
const payload = await page.evaluate(() => window.__calls.filter((c) => c.cmd === 'create_order').pop().args.input);
t.check('payload decimal como string', payload.quantity === '50' && payload.price === '15' && payload.fees === '5', JSON.stringify(payload));

await order({ asset: 'PETR4 · XP', kind: 'Venda', qty: '1000', price: '15', date: '2026-09-04' });
await t.click('button::-p-text(Registrar ordem)');
t.check('venda acima da posição rejeitada', (await formText()).includes('excede a posição de 150'));
t.check('nada gravado', (await t.rows()).length === 3);
await order({ asset: 'HGLG11 · XP', kind: 'Compra', qty: '10', price: '160', date: '2026-09-05' });
await t.click('button::-p-text(Registrar ordem)');
await waitRows(4);
t.check('erro limpo após sucesso', !(await formText()).includes('excede'));

await t.selectByText('section select', 'HGLG11 · XP');
await t.settle();
t.check('filtro por ativo', (await t.rows()).length === 1 && (await t.rows())[0].includes('HGLG11'));
await t.setValue('section select', '');
await t.settle();

// Editar a venda: 50 -> 40  =>  40×15 − 5 − 40×11,10 = 151
await t.rowButton('Venda', 0);
t.check('ordem em edição', (await page.$eval('form h2', (e) => e.innerText)) === 'Editando ordem');
await t.type('input[placeholder="100"]', '40');
await t.click('button::-p-text(Salvar alterações)');
await page.waitForFunction(() => document.body.innerText.includes('+R$ 151,00'));
t.check('resultado recalculado após edição', true);

// Excluir a 1ª compra é permitido (a 2ª cobre a venda); a 2ª, não.
const deleteRow = async (match) => { await t.rowButton(match, 1); await t.rowButton(match, 1); await t.settle(200); };
await deleteRow('01/09/2026');
t.check('exclusão que não afeta a venda', (await t.rows()).length === 3);
await deleteRow('02/09/2026');
t.check('exclusão que quebra a venda é rejeitada', (await t.main()).includes('excede a posição de 0'));
t.check('ordem continua lá', (await t.rows()).length === 3);
await t.shot('carteira-ordens');

// 4. Custódia
await t.click('button::-p-text(Custódia)');
const m = await t.main();
t.check('total investido', m.includes('R$ 2.326,00'), m.slice(0, 500)); // 60×12,10 + 1.600
t.check('PM PETR4 = 12,10', (await t.rows()).some((r) => r.startsWith('PETR4') && r.includes(' 60 ') && r.includes('R$ 12,10')), JSON.stringify(await t.rows()));
t.check('card de FIIs', m.includes('68,79% da carteira · 1 ativo'));
t.check('rosca por tipo', await t.hasChart('Alocação por tipo de ativo'));
t.check('rosca por ativo', await t.hasChart('Alocação por ativo'));
t.check('legenda por ativo', m.includes('HGLG11 R$ 1.600,00 · 68,79%') && m.includes('PETR4 R$ 726,00 · 31,21%'));
await t.shot('carteira-custodia');

// 5. Ativo com ordens não é excluído
await t.click('button::-p-text(Ativos)');
t.check('contagem de ordens', (await t.rows()).some((r) => r.startsWith('PETR4') && r.includes(' 2 ')));
await t.rowButton('PETR4', 1);
await t.rowButton('PETR4', 1);
t.check('ativo com ordens não é excluído', (await t.main()).includes('O ativo possui ordens registradas'));

// 6. Dashboard
await t.click('a[href="/"]');
await page.waitForFunction(() => document.body.innerText.includes('R$ 2.326,00'));
t.check('dashboard mostra patrimônio', true);
t.check('dashboard sem erros de carga', (await page.$$('[role=alert]')).length === 0, await page.$$eval('[role=alert]', (a) => a.map((x) => x.innerText).join(' | ')));
t.check('dashboard: barras de 12 meses', await t.hasChart('Receitas e despesas dos últimos 12 meses'));
t.check('dashboard: rosca da carteira', await t.hasChart('Alocação por tipo de ativo'));
await t.shot('dashboard');

await t.finish();
