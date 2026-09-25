// Ponte mínima com o ECharts (frontend/vendor/echarts.min.js, carregado pelo index.html).
//
// Regras (ver docs/graficos.md):
// * Valores chegam do Rust como STRING decimal; aqui só viram Number para desenhar.
// * Rótulos e textos de tooltip ("R$ 1.234,56 · 75,00%") já chegam formatados do Rust.
// * Sem estado de negócio: cada chamada redesenha o gráfico do elemento.
//
// Tema claro/escuro acompanha `prefers-color-scheme`, inclusive quando muda em tempo real.

const dark = window.matchMedia('(prefers-color-scheme: dark)');
const charts = new Map(); // elemento -> { chart, render, observer }

function palette() {
  return dark.matches
    ? { text: '#cbd5e1', muted: '#64748b', grid: '#1e293b', tooltipBg: '#0f172a', border: '#0f172a' }
    : { text: '#334155', muted: '#94a3b8', grid: '#e2e8f0', tooltipBg: '#ffffff', border: '#ffffff' };
}

function chartFor(el) {
  let entry = charts.get(el);
  if (!entry) {
    const chart = window.echarts.init(el, null, { renderer: 'canvas' });
    const observer = new ResizeObserver(() => chart.resize());
    observer.observe(el);
    entry = { chart, observer, render: null };
    charts.set(el, entry);
  }
  return entry;
}

function draw(el, render) {
  if (!window.echarts) return false; // biblioteca ausente: a legenda HTML continua mostrando os dados
  const entry = chartFor(el);
  entry.render = render;
  entry.chart.setOption(render(palette()), { notMerge: true });
  return true;
}

dark.addEventListener('change', () => {
  for (const entry of charts.values()) entry.chart.setOption(entry.render(palette()), { notMerge: true });
});

function tooltip(p) {
  return {
    backgroundColor: p.tooltipBg,
    borderColor: p.grid,
    textStyle: { color: p.text, fontSize: 12 },
    extraCssText: 'box-shadow: 0 4px 12px rgb(0 0 0 / 0.12); border-radius: 8px;',
  };
}

/** Rosca. slices: [{ label, value: "1234.56", caption, color }] */
export function renderPie(el, slices) {
  return draw(el, (p) => ({
    animationDuration: 300,
    tooltip: { ...tooltip(p), trigger: 'item', formatter: (i) => `<b>${i.name}</b><br>${i.data.caption}` },
    series: [
      {
        type: 'pie',
        radius: ['58%', '88%'],
        avoidLabelOverlap: true,
        label: { show: false },
        itemStyle: { borderColor: p.border, borderWidth: 2 },
        emphasis: { scale: true, scaleSize: 4 },
        data: slices.map((s) => ({ name: s.label, value: Number(s.value), caption: s.caption, itemStyle: { color: s.color } })),
      },
    ],
  }));
}

/**
 * Barras agrupadas. data: { categories: ["Set/26"], series: [{ name, color, values: ["1.5"], captions: ["R$ 1,50"] }] }
 */
export function renderBars(el, data) {
  return draw(el, (p) => ({
    animationDuration: 300,
    grid: { left: 8, right: 8, top: 24, bottom: 4, containLabel: true },
    legend: { top: 0, right: 0, textStyle: { color: p.text, fontSize: 12 }, itemWidth: 10, itemHeight: 10 },
    tooltip: {
      ...tooltip(p),
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      formatter: (items) => `<b>${items[0].axisValue}</b><br>${items.map((i) => `${i.marker} ${i.seriesName}: ${i.data.caption}`).join('<br>')}`,
    },
    xAxis: { type: 'category', data: data.categories, axisLabel: { color: p.muted }, axisLine: { lineStyle: { color: p.grid } }, axisTick: { show: false } },
    yAxis: { type: 'value', axisLabel: { color: p.muted, formatter: (v) => (v >= 1000 ? `${v / 1000}k` : v) }, splitLine: { lineStyle: { color: p.grid } } },
    series: data.series.map((s) => ({
      name: s.name,
      type: 'bar',
      barMaxWidth: 18,
      itemStyle: { color: s.color, borderRadius: [4, 4, 0, 0] },
      data: s.values.map((v, i) => ({ value: Number(v), caption: s.captions[i] })),
    })),
  }));
}

export function dispose(el) {
  const entry = charts.get(el);
  if (!entry) return;
  entry.observer.disconnect();
  entry.chart.dispose();
  charts.delete(el);
}
