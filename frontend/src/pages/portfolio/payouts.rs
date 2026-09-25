use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::chrono::{Datelike, NaiveDate};
use shared::format::{MONTHS_PT, decimal_to_input, format_brl, format_date_br, format_percent};
use shared::payout::{
    IrpfPayoutLine, PayoutTotals, dividend_limit_alerts, irpf_payouts, monthly_payouts, payouts_by_ticker,
    suggested_withholding,
};
use shared::{Asset, Money, NewPayout, Payout, PayoutKind, YearMonth};

use super::{PortfolioState, Tab};
use crate::components::allocation::asset_type_color;
use crate::components::chart::{BarChart, BarData, BarSeries, PALETTE};
use crate::components::form::{
    DeleteButton, EditButton, INPUT, LABEL, PRIMARY_BUTTON, SECONDARY_BUTTON, SegmentOption, Segmented, form_card_class,
};
use crate::components::page::{Card, ErrorBanner, StatCard};
use crate::components::table::{ROW, ROW_EDITING, TABLE_CARD, TD, TH};
use crate::ipc;
use crate::util::{current_year, today};

const KIND_COLORS: [(PayoutKind, &str); 4] = [
    (PayoutKind::Dividend, PALETTE[0]),
    (PayoutKind::Jcp, PALETTE[2]),
    (PayoutKind::FiiIncome, PALETTE[1]),
    (PayoutKind::Other, PALETTE[5]),
];

fn asset_label(asset: &Asset) -> String {
    format!("{} · {}", asset.ticker, asset.broker)
}

#[component]
pub(super) fn PayoutsTab(state: PortfolioState) -> impl IntoView {
    let year = RwSignal::new(current_year());
    let editing = RwSignal::new(None::<Payout>);
    let action_error = RwSignal::new(None::<String>);

    let year_payouts = Signal::derive(move || {
        let y = year.get();
        state.payouts.with(|ps| ps.iter().filter(|p| p.date.year() == y).cloned().collect::<Vec<_>>())
    });
    let totals = Memo::new(move |_| year_payouts.with(|ps| PayoutTotals::of(ps)));
    let alerts = Signal::derive(move || {
        let y = year.get();
        state.payouts.with(|ps| state.assets.with(|a| dividend_limit_alerts(ps, a)))
            .into_iter()
            .filter(|(m, _, _)| m.year == y)
            .collect::<Vec<_>>()
    });

    let bars = Signal::derive(move || {
        let y = year.get();
        let months: Vec<YearMonth> = (1..=12).map(|m| YearMonth { year: y, month: m }).collect();
        let monthly = year_payouts.with(|ps| monthly_payouts(ps, &months));
        BarData {
            categories: months.iter().map(|m| MONTHS_PT[(m.month - 1) as usize][..3].to_string()).collect(),
            series: KIND_COLORS
                .iter()
                .filter(|(kind, _)| monthly.iter().any(|(_, t)| !t.kind(*kind).is_zero()))
                .map(|&(kind, color)| BarSeries {
                    name: kind.label_pt().into(),
                    color: color.into(),
                    values: monthly.iter().map(|(_, t)| t.kind(kind).0.to_string()).collect(),
                    captions: monthly.iter().map(|(_, t)| format_brl(t.kind(kind).0)).collect(),
                })
                .collect(),
        }
    });

    let year_button = "grid size-9 place-items-center rounded-lg text-slate-500 hover:bg-slate-100 hover:text-slate-900 dark:hover:bg-slate-800 dark:hover:text-white";

    view! {
        <ErrorBanner message=state.load_error />

        <div class="mb-4 flex items-center justify-end gap-1">
            <button type="button" class=year_button aria-label="Ano anterior" on:click=move |_| year.update(|y| *y -= 1)>"‹"</button>
            <span class="tabular min-w-16 text-center text-sm font-medium" data-testid="payout-year">{year}</span>
            <button type="button" class=year_button aria-label="Próximo ano" on:click=move |_| year.update(|y| *y += 1)>"›"</button>
        </div>

        {move || {
            alerts.get().into_iter().map(|(month, company, total)| view! {
                <div role="status" class="mb-4 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200">
                    {format!(
                        "{company}: {} em dividendos em {}. Acima de R$ 50 mil no mês de uma mesma empresa há retenção de IR na fonte (Lei 15.270/2025). Informe o valor retido que consta no informe de rendimentos.",
                        format_brl(total.0),
                        month.label_pt(),
                    )}
                </div>
            }).collect_view()
        }}

        <div class="mb-6 grid grid-cols-2 gap-4 xl:grid-cols-4">
            <StatCard
                label="Líquido no ano"
                value=move || format_brl(totals.get().net().0)
                hint=move || format!("Bruto {} · IR retido {}", format_brl(totals.get().gross.0), format_brl(totals.get().withheld.0))
            />
            <StatCard label="Dividendos" value=move || format_brl(totals.get().kind(PayoutKind::Dividend).0) hint="Isentos" />
            <StatCard label="JCP (líquido)" value=move || format_brl(totals.get().kind(PayoutKind::Jcp).0) hint="15% retido na fonte" />
            <StatCard label="Rendimentos de FII" value=move || format_brl(totals.get().kind(PayoutKind::FiiIncome).0) hint="Isentos" />
        </div>

        <Show when=move || !year_payouts.with(Vec::is_empty)>
            <Card class="mb-6">
                <h2 class="mb-2 text-sm font-semibold">{move || format!("Proventos líquidos por mês · {}", year.get())}</h2>
                <BarChart data=bars label="Proventos líquidos por mês" />
            </Card>
        </Show>

        <Show
            when=move || !state.assets.with(Vec::is_empty)
            fallback=move || view! {
                <div class="mb-6 rounded-xl border border-dashed border-slate-300 p-8 text-center text-sm text-slate-600 dark:border-slate-700 dark:text-slate-400">
                    <p>"Para registrar proventos, cadastre primeiro o ativo."</p>
                    <button type="button" class=format!("{PRIMARY_BUTTON} mt-4") on:click=move |_| state.tab.set(Tab::Assets)>"Cadastrar ativo"</button>
                </div>
            }
        >
            <PayoutForm state editing year />
        </Show>

        <section class=format!("{TABLE_CARD} mb-6")>
            <div class="border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <h2 class="text-sm font-semibold">
                    {move || format!("Proventos de {} ", year.get())}
                    <span class="font-normal text-slate-500">{move || format!("· {}", year_payouts.with(Vec::len))}</span>
                </h2>
            </div>
            <div class="px-4 pt-4 empty:hidden">
                <ErrorBanner message=action_error />
            </div>
            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=TH>"Pagamento"</th>
                        <th class=TH>"Ativo"</th>
                        <th class=TH>"Tipo"</th>
                        <th class=format!("{TH} text-right")>"Bruto"</th>
                        <th class=format!("{TH} text-right")>"IR retido"</th>
                        <th class=format!("{TH} text-right")>"Líquido"</th>
                        <th class=format!("{TH} text-right")><span class="sr-only">"Ações"</span></th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    <For
                        each=move || year_payouts.get()
                        key=|p| p.clone()
                        children=move |payout| view! { <PayoutRow payout state editing action_error /> }
                    />
                </tbody>
            </table>
            <Show when=move || year_payouts.with(Vec::is_empty)>
                <p class="px-4 py-10 text-center text-sm text-slate-500">{move || format!("Nenhum provento em {}.", year.get())}</p>
            </Show>
        </section>

        <Show when=move || !year_payouts.with(Vec::is_empty)>
            <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
                <ByTicker state year_payouts />
                <IrpfSummary state year year_payouts />
            </div>
        </Show>
    }
}

#[component]
fn PayoutForm(state: PortfolioState, editing: RwSignal<Option<Payout>>, year: RwSignal<i32>) -> impl IntoView {
    let asset_id = RwSignal::new(String::new());
    let kind = RwSignal::new(PayoutKind::Dividend);
    let date = RwSignal::new(today().to_string());
    let gross = RwSignal::new(String::new());
    let withheld = RwSignal::new(String::new());
    // Enquanto o usuário não mexer no IR retido, ele acompanha a sugestão (15% no JCP).
    let withheld_touched = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);

    let selected_asset = move || asset_id.get().parse::<i64>().ok().and_then(|id| state.asset(id));
    let allowed = move || selected_asset().map(|a| PayoutKind::allowed_for(a.asset_type).to_vec()).unwrap_or_default();

    // Pré-seleciona o primeiro ativo e mantém o tipo compatível com ele.
    Effect::new(move |_| {
        let assets = state.assets.get();
        if !assets.iter().any(|a| a.id.to_string() == asset_id.get_untracked())
            && let Some(first) = assets.first()
        {
            asset_id.set(first.id.to_string());
        }
    });
    Effect::new(move |_| {
        let allowed = allowed();
        if !allowed.is_empty() && !allowed.contains(&kind.get_untracked()) {
            kind.set(allowed[0]);
        }
    });
    Effect::new(move |_| {
        let suggestion = Money::parse_br(&gross.get()).ok().map(|g| suggested_withholding(kind.get(), g));
        if !withheld_touched.get_untracked() {
            withheld.set(suggestion.filter(|s| !s.is_zero()).map(|s| decimal_to_input(s.0)).unwrap_or_default());
        }
    });
    Effect::new(move |_| {
        if let Some(p) = editing.get() {
            asset_id.set(p.asset_id.to_string());
            kind.set(p.kind);
            date.set(p.date.to_string());
            gross.set(decimal_to_input(p.gross.0));
            withheld_touched.set(true);
            withheld.set(if p.withheld.is_zero() { String::new() } else { decimal_to_input(p.withheld.0) });
            error.set(None);
        }
    });

    let clear = move || {
        gross.set(String::new());
        withheld.set(String::new());
        withheld_touched.set(false);
        error.set(None);
    };

    let parse = move || -> Result<NewPayout, String> {
        let asset_id = asset_id.get().parse::<i64>().map_err(|_| "Selecione um ativo.".to_string())?;
        let gross = Money::parse_br(&gross.get()).map_err(|_| "Valor bruto inválido. Ex.: 123,45".to_string())?;
        let withheld_text = withheld.get();
        let withheld = if withheld_text.trim().is_empty() {
            Money::ZERO
        } else {
            Money::parse_br(&withheld_text).map_err(|_| "IR retido inválido.".to_string())?
        };
        let date = NaiveDate::parse_from_str(&date.get(), "%Y-%m-%d").map_err(|_| "Data inválida.".to_string())?;
        NewPayout { asset_id, kind: kind.get(), date, gross, withheld }.validated().map_err(|e| e.message)
    };
    let net_preview = move || parse().ok().map(|p| format!("Líquido: {}", format_brl(p.gross.0 - p.withheld.0)));

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        if saving.get_untracked() {
            return;
        }
        let input = match untrack(parse) {
            Ok(input) => input,
            Err(msg) => return error.set(Some(msg)),
        };
        saving.set(true);
        error.set(None);
        let editing_id = editing.get_untracked().map(|p| p.id);
        spawn_local(async move {
            let result = match editing_id {
                Some(id) => ipc::update_payout(id, input).await,
                None => ipc::create_payout(input).await,
            };
            saving.set(false);
            match result {
                Ok(saved) => {
                    editing.set(None);
                    clear();
                    year.set(saved.date.year());
                    state.reload();
                }
                Err(msg) => error.set(Some(msg)),
            }
        });
    };

    let is_editing = move || editing.with(Option::is_some);

    view! {
        <form on:submit=submit class=move || form_card_class(is_editing())>
            <h2 class="mb-4 text-sm font-semibold">{move || if is_editing() { "Editando provento" } else { "Novo provento" }}</h2>
            <ErrorBanner message=error />
            <div class="grid grid-cols-2 gap-3 lg:grid-cols-12">
                <label class="col-span-2 lg:col-span-3">
                    <span class=LABEL>"Ativo"</span>
                    <select class=INPUT bind:value=asset_id>
                        {move || state.assets.get().into_iter().map(|a| view! { <option value=a.id.to_string()>{asset_label(&a)}</option> }).collect_view()}
                    </select>
                </label>
                <div class="col-span-2 lg:col-span-3">
                    <span class=LABEL>"Tipo"</span>
                    {move || {
                        let options = allowed().into_iter().map(|k| SegmentOption::new(k, k.label_pt())).collect();
                        view! { <Segmented value=kind options /> }
                    }}
                </div>
                <label class="lg:col-span-2">
                    <span class=LABEL>"Pagamento"</span>
                    <input class=INPUT type="date" required bind:value=date />
                </label>
                <label class="lg:col-span-2">
                    <span class=LABEL>"Valor bruto (R$)"</span>
                    <input class=format!("{INPUT} tabular text-right") inputmode="decimal" placeholder="0,00" aria-label="Valor bruto" autocomplete="off" bind:value=gross />
                </label>
                <label class="lg:col-span-2">
                    <span class=LABEL>"IR retido (R$)"</span>
                    <input
                        class=format!("{INPUT} tabular text-right")
                        inputmode="decimal"
                        placeholder="0,00"
                        aria-label="IR retido"
                        autocomplete="off"
                        bind:value=withheld
                        on:input=move |_| withheld_touched.set(true)
                    />
                </label>
            </div>
            <div class="mt-4 flex items-center justify-between gap-2">
                <p class="tabular text-sm text-slate-500 dark:text-slate-400">
                    {net_preview}
                    {move || (kind.get() == PayoutKind::Jcp).then_some(" · JCP: 15% retido na fonte")}
                </p>
                <div class="flex gap-2">
                    <Show when=is_editing>
                        <button type="button" class=SECONDARY_BUTTON on:click=move |_| { editing.set(None); clear(); }>"Cancelar"</button>
                    </Show>
                    <button type="submit" class=PRIMARY_BUTTON disabled=move || saving.get()>
                        {move || match (saving.get(), is_editing()) {
                            (true, _) => "Salvando…",
                            (false, true) => "Salvar alterações",
                            (false, false) => "Registrar provento",
                        }}
                    </button>
                </div>
            </div>
        </form>
    }
}

#[component]
fn PayoutRow(
    payout: Payout,
    state: PortfolioState,
    editing: RwSignal<Option<Payout>>,
    action_error: RwSignal<Option<String>>,
) -> impl IntoView {
    let id = payout.id;
    let is_editing = move || editing.with(|e| e.as_ref().is_some_and(|p| p.id == id));
    let asset = move || state.asset(payout.asset_id);
    let net = payout.net();
    let edit = {
        let payout = payout.clone();
        move |()| editing.set(Some(payout.clone()))
    };
    let delete = move |()| {
        spawn_local(async move {
            match ipc::delete_payout(id).await {
                Ok(()) => {
                    action_error.set(None);
                    if is_editing() {
                        editing.set(None);
                    }
                    state.reload();
                }
                Err(msg) => action_error.set(Some(msg)),
            }
        });
    };

    view! {
        <tr class=move || if is_editing() { ROW_EDITING } else { ROW }>
            <td class=format!("{TD} tabular text-slate-600 dark:text-slate-400")>{format_date_br(payout.date)}</td>
            <td class=TD>
                {move || asset().map(|a| view! {
                    <span class="flex items-center gap-2">
                        <span class=format!("size-2 rounded-full {}", asset_type_color(a.asset_type))></span>
                        <span class="font-medium">{a.ticker}</span>
                        <span class="text-xs text-slate-400">{a.broker}</span>
                    </span>
                })}
            </td>
            <td class=TD>{payout.kind.label_pt()}</td>
            <td class=format!("{TD} tabular text-right")>{format_brl(payout.gross.0)}</td>
            <td class=format!("{TD} tabular text-right text-slate-500")>{format_brl(payout.withheld.0)}</td>
            <td class=format!("{TD} tabular text-right font-medium text-emerald-600 dark:text-emerald-400")>{format_brl(net.0)}</td>
            <td class=format!("{TD} text-right whitespace-nowrap")>
                <EditButton on_click=edit />
                <DeleteButton on_confirm=delete />
            </td>
        </tr>
    }
}

/// Líquido por ativo no ano e yield on cost (sobre o custo atual da posição).
#[component]
fn ByTicker(state: PortfolioState, year_payouts: Signal<Vec<Payout>>) -> impl IntoView {
    let rows = move || {
        let positions = state.portfolio.get().map(|p| p.positions).unwrap_or_default();
        year_payouts.with(|ps| state.assets.with(|a| payouts_by_ticker(ps, a, &positions)))
    };
    view! {
        <section class=TABLE_CARD>
            <div class="border-b border-slate-200 px-4 py-3 dark:border-slate-800">
                <h2 class="text-sm font-semibold">"Por ativo"</h2>
            </div>
            <table class="w-full text-sm">
                <thead class="bg-slate-50 dark:bg-slate-950/40">
                    <tr>
                        <th class=TH>"Ativo"</th>
                        <th class=format!("{TH} text-right")>"Líquido"</th>
                        <th class=format!("{TH} text-right")>
                            <span title="Líquido recebido no ano ÷ custo atual da posição">"Yield on cost"</span>
                        </th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    {move || rows().into_iter().map(|r| view! {
                        <tr class=ROW>
                            <td class=TD>
                                <span class="flex items-center gap-2 font-medium">
                                    <span class=format!("size-2 rounded-full {}", asset_type_color(r.asset_type))></span>
                                    {r.ticker}
                                </span>
                            </td>
                            <td class=format!("{TD} tabular text-right")>{format_brl(r.net.0)}</td>
                            <td class=format!("{TD} tabular text-right text-slate-600 dark:text-slate-400")>
                                {r.yield_on_cost.map(format_percent).unwrap_or_else(|| "—".into())}
                            </td>
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
        </section>
    }
}

/// Totais por ficha do IRPF (declaração do ano seguinte).
#[component]
fn IrpfSummary(state: PortfolioState, year: RwSignal<i32>, year_payouts: Signal<Vec<Payout>>) -> impl IntoView {
    let lines = move || year_payouts.with(|ps| state.assets.with(|a| irpf_payouts(ps, a, year.get())));
    let groups = move || {
        let lines = lines();
        PayoutKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let of_kind: Vec<IrpfPayoutLine> = lines.iter().filter(|l| l.kind == kind).cloned().collect();
                (!of_kind.is_empty()).then_some((kind, of_kind))
            })
            .collect::<Vec<_>>()
    };
    view! {
        <Card>
            <h2 class="text-sm font-semibold">{move || format!("Para a declaração de {} (ano-calendário {})", year.get() + 1, year.get())}</h2>
            <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">
                "Informe o CNPJ da fonte pagadora conforme o informe de rendimentos de cada empresa ou fundo."
            </p>
            <div class="mt-4 space-y-4">
                {move || groups().into_iter().map(|(kind, lines)| {
                    let total: shared::rust_decimal::Decimal = lines.iter().map(|l| l.net.0).sum();
                    view! {
                        <div>
                            <p class="flex justify-between text-sm font-medium">
                                <span>{kind.label_pt()}</span>
                                <span class="tabular">{format_brl(total)}</span>
                            </p>
                            <p class="text-xs text-slate-500 dark:text-slate-400">{kind.irpf_hint()}</p>
                            <ul class="mt-1 space-y-0.5 text-sm text-slate-600 dark:text-slate-400">
                                {lines.into_iter().map(|l| view! {
                                    <li class="flex justify-between">
                                        <span>{l.ticker}</span>
                                        <span class="tabular">
                                            {if l.withheld.is_zero() {
                                                format_brl(l.net.0)
                                            } else {
                                                format!("{} (bruto {} − IR {})", format_brl(l.net.0), format_brl(l.gross.0), format_brl(l.withheld.0))
                                            }}
                                        </span>
                                    </li>
                                }).collect_view()}
                            </ul>
                        </div>
                    }
                }).collect_view()}
            </div>
        </Card>
    }
}
