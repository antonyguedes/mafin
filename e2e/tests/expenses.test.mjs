// Tela de Gastos: resumo, validação, criar/editar/excluir, filtros e navegação de mês.
import { currentMonth, monthLabel as label, openApp } from '../lib.mjs';

function backend() {
  const db = {
    nextId: 5,
    rows: [
      { id: 1, kind: 'income', amount: '8500', date: '2026-09-05', category: 'Salário', description: 'Salário setembro' },
      { id: 2, kind: 'expense', amount: '2200', date: '2026-09-10', category: 'Moradia', description: 'Aluguel' },
      { id: 3, kind: 'expense', amount: '0.1', date: '2026-09-12', category: 'Mercado', description: null },
      { id: 4, kind: 'expense', amount: '150.2', date: '2026-08-20', category: 'Lazer', description: 'Cinema' },
    ],
  };
  const inPeriod = (date, p) => !p || (Number(date.slice(0, 4)) === p.year && Number(date.slice(5, 7)) === p.month);
  const find = (id) => {
    const i = db.rows.findIndex((t) => t.id === id);
    if (i < 0) throw `Lançamento #${id} não encontrado`;
    return i;
  };
  return {
    ping: () => ({ reply: 'pong', tripled: '0', backend_version: '0.1.0' }),
    list_transactions: ({ filter }) =>
      db.rows
        .filter((t) => inPeriod(t.date, filter?.period) && (!filter?.kind || t.kind === filter.kind))
        .sort((a, b) => (a.date === b.date ? b.id - a.id : a.date < b.date ? 1 : -1)),
    create_transaction: ({ input }) => {
      const row = { id: db.nextId++, ...input, description: input.description || null };
      db.rows.push(row);
      return row;
    },
    update_transaction: ({ id, input }) => (db.rows[find(id)] = { id, ...input, description: input.description || null }),
    delete_transaction: ({ id }) => void db.rows.splice(find(id), 1),
    list_categories: () => [...new Set(db.rows.map((t) => t.category))].sort(),
  };
}

const t = await openApp('/gastos', backend);
const { page } = t;
const cards = () => page.$$eval('main section p.tabular', (els) => els.map((e) => e.innerText.trim()));
const formText = () => page.$eval('form', (f) => f.innerText);
const now = currentMonth();
t.check('abre no mês atual', (await t.monthLabel()) === label(now.year, now.month), await t.monthLabel());
// Os dados do cenário são de set/2026: navega até lá (o teste não depende da data de hoje).
await t.gotoMonth(2026, 9);
await page.waitForSelector('tbody tr');
await t.settle();

// 1. Carga inicial
t.check('3 linhas de setembro', (await t.rows()).length === 3, JSON.stringify(await t.rows()));
t.check('resumo exato (0,1 somado sem float)', JSON.stringify(await cards()) === JSON.stringify(['R$ 8.500,00', 'R$ 2.200,10', 'R$ 6.299,90']), JSON.stringify(await cards()));
t.check('ordem por data desc', (await t.rows())[0].startsWith('12/09/2026'));
t.check('gráfico de despesas por categoria', await t.hasChart('Despesas por categoria'));
t.check('legenda do gráfico formatada', (await t.main()).includes('Moradia R$ 2.200,00 · 100,00%'), (await t.main()).slice(-300));
await t.shot('gastos-1-inicial');

// 2. Validação no cliente
await t.type('input[placeholder="0,00"]', 'abc');
await t.type('input[list="expense-categories"]', 'Mercado');
await t.click('button[type=submit]');
t.check('erro de valor inválido', (await formText()).includes('Valor inválido'));
await t.type('input[placeholder="0,00"]', '10,005');
await t.click('button[type=submit]');
t.check('erro de centavos', (await formText()).includes('Use no máximo 2 casas decimais'));

// 3. Criar receita com separador de milhar
await t.click('button::-p-text(Receita)');
await t.type('input[placeholder="0,00"]', '1.234,56');
await t.type('input[list="expense-categories"]', 'Freelance');
await t.type('input[placeholder="Ex.: Compra da semana"]', 'Projeto site');
await t.click('button[type=submit]');
await page.waitForFunction(() => document.querySelectorAll('tbody tr').length === 4);
const created = await page.evaluate(() => window.__calls.filter((c) => c.cmd === 'create_transaction').pop().args.input);
t.check('payload com Decimal como string', created.amount === '1234.56' && created.kind === 'income', JSON.stringify(created));
t.check('resumo após criar', JSON.stringify(await cards()) === JSON.stringify(['R$ 9.734,56', 'R$ 2.200,10', 'R$ 7.534,46']), JSON.stringify(await cards()));
t.check('formulário limpo', (await page.$eval('input[placeholder="0,00"]', (e) => e.value)) === '' && !(await formText()).includes('inválido'));
t.check('categoria nova sugerida', (await page.$$eval('#expense-categories option', (o) => o.map((x) => x.value))).includes('Freelance'));

// 4. Editar
await t.rowButton('Aluguel', 0);
t.check('form em modo edição', (await page.$eval('form h2', (e) => e.innerText)) === 'Editando lançamento');
t.check('valor carregado', (await page.$eval('input[placeholder="0,00"]', (e) => e.value)) === '2200');
await t.type('input[placeholder="0,00"]', '2400');
await t.click('button::-p-text(Salvar alterações)');
await page.waitForFunction(() => document.body.innerText.includes('R$ 2.400,00'));
t.check('linha atualizada', (await t.rows()).some((r) => r.includes('Aluguel') && r.includes('R$ 2.400,00')));
t.check('saiu do modo edição', (await page.$eval('form h2', (e) => e.innerText)) === 'Novo lançamento');

// 5. Excluir com confirmação
const count = (await t.rows()).length;
await t.rowButton('Mercado', 1);
t.check('pede confirmação', (await t.rows()).some((r) => r.includes('Mercado') && r.includes('Confirmar')));
t.check('ainda não excluiu', (await t.rows()).length === count);
await t.rowButton('Mercado', 1);
await page.waitForFunction((n) => document.querySelectorAll('tbody tr').length === n - 1, {}, count);
t.check('excluiu após confirmar', !(await t.rows()).some((r) => r.includes('Mercado')));

// 6. Filtro por tipo
await t.click('button::-p-text(Despesas)');
t.check('só despesas', (await t.rows()).length === 1 && (await t.rows())[0].includes('−'), JSON.stringify(await t.rows()));
t.check('resumo não muda com filtro', (await cards())[0] === 'R$ 9.734,56');
await t.click('button::-p-text(Todos)');

// 7. Navegação de mês
await t.click('button[aria-label="Mês anterior"]');
await page.waitForFunction(() => document.body.innerText.includes('Cinema'));
t.check('agosto mostra 1 linha', (await t.rows()).length === 1 && (await t.monthLabel()) === 'Agosto de 2026');
t.check('data padrão move para agosto', (await page.$eval('input[type=date]', (e) => e.value)) === '2026-08-01');
await t.click('button[aria-label="Mês anterior"]');
t.check('mês vazio', (await t.main()).includes('Nenhum lançamento em Julho de 2026.'));
await t.click('button::-p-text(Mês atual)');
t.check('botão "Mês atual"', (await t.monthLabel()) === label(now.year, now.month));
await t.gotoMonth(2026, 9);
await page.waitForFunction(() => document.querySelectorAll('tbody tr').length === 3);

// 8. Lançar em outro mês pula para ele
await t.type('input[placeholder="0,00"]', '99,90');
await t.type('input[list="expense-categories"]', 'Lazer');
await t.setValue('input[type=date]', '2026-10-03');
await t.click('button[type=submit]');
await page.waitForFunction(() => document.body.innerText.includes('Outubro de 2026'));
t.check('pulou para outubro', (await t.rows()).length === 1 && (await t.rows())[0].includes('03/10/2026'));

await t.finish();
