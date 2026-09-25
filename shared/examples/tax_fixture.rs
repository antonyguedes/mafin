//! Gera o cenário usado pelo teste E2E da tela de IR, calculado pelo motor REAL.
//!
//! `cargo run -p shared --example tax_fixture -- <saida.json>`
//!
//! Cenário (2026):
//! * Jul: compra 1.000 PETR4; vende 400 com prejuízo, em mês isento (vendas < 20 mil).
//! * Ago: vende 600 PETR4 (vendas > 20 mil, compensa o prejuízo de julho) e 50 HGLG11.
//! * Set: compra e vende 100 VALE3 no mesmo dia (day trade, 20%) e vende 5 HGLG11.
//! * IRRF informado em agosto: R$ 1,49.

use std::collections::BTreeMap;
use std::{env, fs};

use rust_decimal_macros::dec;
use shared::chrono::NaiveDate;
use shared::tax::tax_report;
use shared::{Asset, AssetType, Money, Order, OrderKind, Quantity, YearMonth, build_portfolio};

fn main() {
    let out = env::args().nth(1).expect("uso: tax_fixture <saida.json>");

    let asset = |id, ticker: &str, asset_type, broker: &str| Asset { id, ticker: ticker.into(), asset_type, broker: broker.into() };
    let assets = vec![
        asset(1, "PETR4", AssetType::Stock, "XP"),
        asset(2, "HGLG11", AssetType::Fii, "XP"),
        asset(3, "VALE3", AssetType::Stock, "Rico"),
    ];

    let order = |id, asset_id, kind, qty, price, fees, (month, day)| Order {
        id,
        asset_id,
        kind,
        quantity: Quantity(qty),
        price: Money(price),
        fees: Money(fees),
        date: NaiveDate::from_ymd_opt(2026, month, day).unwrap(),
    };
    use OrderKind::{Buy, Sell};
    let orders = vec![
        order(1, 1, Buy, dec!(1000), dec!(30), dec!(10), (7, 1)),
        order(2, 1, Sell, dec!(400), dec!(27), dec!(5), (7, 15)),
        order(3, 2, Buy, dec!(100), dec!(160), dec!(0), (8, 1)),
        order(4, 1, Sell, dec!(600), dec!(36), dec!(5), (8, 10)),
        order(5, 2, Sell, dec!(50), dec!(163), dec!(0), (8, 20)),
        order(6, 3, Buy, dec!(100), dec!(60), dec!(0), (9, 15)),
        order(7, 3, Sell, dec!(100), dec!(62), dec!(0), (9, 15)),
        order(8, 2, Sell, dec!(5), dec!(165), dec!(0), (9, 22)),
    ];

    let portfolio = build_portfolio(&assets, &orders).expect("cenário válido");
    let irrf = BTreeMap::from([(YearMonth::new(2026, 8).unwrap(), Money(dec!(1.49)))]);
    let tax = tax_report(&portfolio.sales, &irrf);
    let json = serde_json::json!({ "assets": assets, "orders": orders, "portfolio": portfolio, "tax": tax });
    if let Some(dir) = std::path::Path::new(&out).parent() {
        fs::create_dir_all(dir).expect("falha ao criar diretório");
    }
    fs::write(&out, serde_json::to_string_pretty(&json).unwrap()).expect("falha ao gravar");
    eprintln!("fixture gravada em {out}");
}
