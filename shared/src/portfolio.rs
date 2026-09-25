//! Motor de posição: percorre as ordens em ordem cronológica e calcula, por ticker,
//! quantidade, preço médio (PM) e custo; e, para cada venda, o resultado realizado.
//!
//! Regras (Receita Federal, renda variável):
//! * O PM é consolidado por TICKER, somando todas as corretoras.
//! * Compra: PM = ((Qtd ant. × PM ant.) + (Qtd nova × Preço + Taxas)) / (Qtd ant. + Qtd nova).
//!   Implementado acumulando o custo total (`custo += qtd × preço + taxas`, `PM = custo / qtd`),
//!   que é algebricamente a mesma fórmula e evita arredondar o PM a cada passo.
//! * Venda: o PM não muda; resultado = qtd × preço − taxas − qtd × PM.
//! * Ordens do mesmo dia seguem a ordem de cadastro (id). Day trade tem regra própria (Fase 6).
//!
//! Nada aqui arredonda: o arredondamento para centavos é feito só na exibição/apuração.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::format::{format_date_br, format_decimal_br};
use crate::{Asset, AssetType, Money, Order, OrderKind, Quantity, ValidationError};

/// Posição atual em um ticker (somente quantidade > 0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub ticker: String,
    pub asset_type: AssetType,
    /// Corretoras em que houve ordens deste ticker.
    pub brokers: Vec<String>,
    pub quantity: Quantity,
    pub average_price: Money,
    /// Custo de aquisição da quantidade atual (quantidade × PM).
    pub total_cost: Money,
}

/// Resultado realizado de uma ordem de venda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaleResult {
    pub order_id: i64,
    pub ticker: String,
    pub asset_type: AssetType,
    pub date: NaiveDate,
    pub quantity: Quantity,
    /// Quantidade × preço de venda (base do limite de isenção de R$ 20 mil).
    pub gross: Money,
    pub fees: Money,
    /// Quantidade × PM na data da venda.
    pub cost: Money,
    /// gross − fees − cost. Negativo = prejuízo.
    pub result: Money,
    /// Houve compra do mesmo ticker na mesma data (possível day trade, que tem regra própria).
    pub day_trade: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationSlice {
    pub asset_type: AssetType,
    pub cost: Money,
    /// Participação no custo total, de 0 a 100 (sem arredondamento).
    pub percent: Decimal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Portfolio {
    /// Ordenadas por ticker.
    pub positions: Vec<Position>,
    /// Em ordem cronológica.
    pub sales: Vec<SaleResult>,
    /// Somente tipos com custo > 0, na ordem de `AssetType::ALL`.
    pub allocation: Vec<AllocationSlice>,
    pub total_cost: Money,
}

#[derive(Default)]
struct Book {
    quantity: Decimal,
    cost: Decimal,
    brokers: BTreeSet<String>,
}

/// Calcula a carteira. Falha se os dados forem inconsistentes: venda maior que a posição,
/// quantidade fracionária em ação/FII, ou o mesmo ticker cadastrado com tipos diferentes.
pub fn build_portfolio(assets: &[Asset], orders: &[Order]) -> Result<Portfolio, ValidationError> {
    let mut ticker_types: HashMap<&str, AssetType> = HashMap::new();
    for asset in assets {
        match ticker_types.insert(&asset.ticker, asset.asset_type) {
            Some(previous) if previous != asset.asset_type => {
                return Err(ValidationError::new(
                    "asset_type",
                    &format!(
                        "{} já está cadastrado como {} em outra corretora",
                        asset.ticker,
                        previous.label_pt()
                    ),
                ));
            }
            _ => {}
        }
    }

    let assets_by_id: HashMap<i64, &Asset> = assets.iter().map(|a| (a.id, a)).collect();
    let mut chronological: Vec<&Order> = orders.iter().collect();
    chronological.sort_by_key(|o| (o.date, o.id));

    // (ticker, data) com alguma compra: uma venda nesse par é possível day trade.
    let buy_days: BTreeSet<(&str, NaiveDate)> = orders
        .iter()
        .filter(|o| o.kind == OrderKind::Buy)
        .filter_map(|o| assets_by_id.get(&o.asset_id).map(|a| (a.ticker.as_str(), o.date)))
        .collect();

    let mut books: BTreeMap<&str, Book> = BTreeMap::new();
    let mut sales = Vec::new();

    for order in chronological {
        let asset = assets_by_id.get(&order.asset_id).ok_or_else(|| {
            ValidationError::new("asset_id", &format!("Ordem #{} aponta para um ativo inexistente", order.id))
        })?;
        let qty = order.quantity.0;

        if asset.asset_type != AssetType::FixedIncome && !qty.fract().is_zero() {
            return Err(ValidationError::new(
                "quantity",
                &format!("{} ({}) só aceita quantidades inteiras", asset.ticker, asset.asset_type.label_pt()),
            ));
        }

        let book = books.entry(&asset.ticker).or_default();
        book.brokers.insert(asset.broker.clone());

        match order.kind {
            OrderKind::Buy => {
                book.cost += qty * order.price.0 + order.fees.0;
                book.quantity += qty;
            }
            OrderKind::Sell => {
                if qty > book.quantity {
                    return Err(ValidationError::new(
                        "quantity",
                        &format!(
                            "Venda de {} {} em {} excede a posição de {}",
                            format_decimal_br(qty.normalize(), qty.normalize().scale()),
                            asset.ticker,
                            format_date_br(order.date),
                            format_decimal_br(book.quantity.normalize(), book.quantity.normalize().scale()),
                        ),
                    ));
                }
                // Custo proporcional = qtd × PM, sem materializar o PM arredondado.
                let cost = if qty == book.quantity { book.cost } else { book.cost * qty / book.quantity };
                let gross = qty * order.price.0;
                sales.push(SaleResult {
                    order_id: order.id,
                    ticker: asset.ticker.clone(),
                    asset_type: asset.asset_type,
                    date: order.date,
                    quantity: order.quantity,
                    gross: Money(gross),
                    fees: order.fees,
                    cost: Money(cost),
                    result: Money(gross - order.fees.0 - cost),
                    day_trade: buy_days.contains(&(asset.ticker.as_str(), order.date)),
                });
                book.cost -= cost;
                book.quantity -= qty;
                if book.quantity.is_zero() {
                    book.cost = Decimal::ZERO;
                }
            }
        }
    }

    let positions: Vec<Position> = books
        .into_iter()
        .filter(|(_, book)| book.quantity > Decimal::ZERO)
        .map(|(ticker, book)| Position {
            ticker: ticker.to_owned(),
            asset_type: ticker_types[ticker],
            brokers: book.brokers.into_iter().collect(),
            quantity: Quantity(book.quantity),
            average_price: Money(book.cost / book.quantity),
            total_cost: Money(book.cost),
        })
        .collect();

    let total_cost: Decimal = positions.iter().map(|p| p.total_cost.0).sum();
    let allocation = AssetType::ALL
        .iter()
        .filter_map(|&asset_type| {
            let cost: Decimal = positions.iter().filter(|p| p.asset_type == asset_type).map(|p| p.total_cost.0).sum();
            (cost > Decimal::ZERO).then(|| AllocationSlice {
                asset_type,
                cost: Money(cost),
                percent: cost / total_cost * Decimal::ONE_HUNDRED,
            })
        })
        .collect();

    Ok(Portfolio { positions, sales, allocation, total_cost: Money(total_cost) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn asset(id: i64, ticker: &str, asset_type: AssetType, broker: &str) -> Asset {
        Asset { id, ticker: ticker.into(), asset_type, broker: broker.into() }
    }

    fn order(id: i64, asset_id: i64, kind: OrderKind, qty: Decimal, price: Decimal, fees: Decimal, day: u32) -> Order {
        Order {
            id,
            asset_id,
            kind,
            quantity: Quantity(qty),
            price: Money(price),
            fees: Money(fees),
            date: NaiveDate::from_ymd_opt(2026, 9, day).unwrap(),
        }
    }

    use OrderKind::{Buy, Sell};

    #[test]
    fn average_price_follows_the_spec_formula() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        let orders = [
            order(1, 1, Buy, dec!(100), dec!(10), dec!(10), 1),
            order(2, 1, Buy, dec!(100), dec!(12), dec!(10), 2),
        ];
        let p = build_portfolio(&assets, &orders).unwrap();
        // 1ª: (0 + 100×10 + 10) / 100 = 10,10
        // 2ª: (100×10,10 + 100×12 + 10) / 200 = 11,10
        assert_eq!(p.positions[0].average_price, Money(dec!(11.10)));
        assert_eq!(p.positions[0].quantity, Quantity(dec!(200)));
        assert_eq!(p.total_cost, Money(dec!(2220)));
    }

    #[test]
    fn sale_uses_current_average_price_and_keeps_it() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        let orders = [
            order(1, 1, Buy, dec!(100), dec!(10), dec!(10), 1),
            order(2, 1, Buy, dec!(100), dec!(12), dec!(10), 2),
            order(3, 1, Sell, dec!(50), dec!(15), dec!(5), 3),
        ];
        let p = build_portfolio(&assets, &orders).unwrap();
        let sale = &p.sales[0];
        assert_eq!(sale.gross, Money(dec!(750)));
        assert_eq!(sale.cost, Money(dec!(555))); // 50 × 11,10
        assert_eq!(sale.result, Money(dec!(190))); // 750 − 5 − 555
        assert_eq!(p.positions[0].quantity, Quantity(dec!(150)));
        assert_eq!(p.positions[0].average_price, Money(dec!(11.10)));
    }

    #[test]
    fn loss_is_negative_result() {
        let assets = [asset(1, "HGLG11", AssetType::Fii, "XP")];
        let orders = [order(1, 1, Buy, dec!(10), dec!(160), dec!(0), 1), order(2, 1, Sell, dec!(10), dec!(150), dec!(1), 2)];
        let p = build_portfolio(&assets, &orders).unwrap();
        assert_eq!(p.sales[0].result, Money(dec!(-101)));
        assert!(p.positions.is_empty());
    }

    #[test]
    fn selling_everything_resets_average_price() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        let orders = [
            order(1, 1, Buy, dec!(3), dec!(10), dec!(1), 1), // PM 10,333...
            order(2, 1, Sell, dec!(3), dec!(11), dec!(0), 2),
            order(3, 1, Buy, dec!(10), dec!(20), dec!(0), 3),
        ];
        let p = build_portfolio(&assets, &orders).unwrap();
        assert_eq!(p.sales[0].cost, Money(dec!(31)));
        assert_eq!(p.positions[0].average_price, Money(dec!(20)));
    }

    #[test]
    fn repeating_decimals_stay_consistent() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        let orders = [order(1, 1, Buy, dec!(3), dec!(10), dec!(1), 1), order(2, 1, Sell, dec!(1), dec!(10), dec!(0), 2)];
        let p = build_portfolio(&assets, &orders).unwrap();
        let pos = &p.positions[0];
        // Custo restante + custo vendido = custo total pago (31), sem "sumir" centavo.
        assert_eq!((pos.total_cost.0 + p.sales[0].cost.0).round_dp(20), dec!(31));
        assert_eq!(pos.average_price.round_dp(2), dec!(10.33));
    }

    #[test]
    fn consolidates_ticker_across_brokers() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP"), asset(2, "PETR4", AssetType::Stock, "Rico")];
        let orders = [order(1, 1, Buy, dec!(100), dec!(10), dec!(0), 1), order(2, 2, Buy, dec!(100), dec!(20), dec!(0), 2)];
        let p = build_portfolio(&assets, &orders).unwrap();
        assert_eq!(p.positions.len(), 1);
        assert_eq!(p.positions[0].average_price, Money(dec!(15)));
        assert_eq!(p.positions[0].brokers, ["Rico", "XP"]);
    }

    #[test]
    fn flags_same_day_buy_and_sell() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        let orders = [
            order(1, 1, Buy, dec!(100), dec!(10), dec!(0), 1),
            order(2, 1, Sell, dec!(50), dec!(11), dec!(0), 2),
            order(3, 1, Buy, dec!(10), dec!(10), dec!(0), 3),
            order(4, 1, Sell, dec!(10), dec!(12), dec!(0), 3),
        ];
        let p = build_portfolio(&assets, &orders).unwrap();
        assert_eq!(p.sales.iter().map(|s| s.day_trade).collect::<Vec<_>>(), [false, true]);
    }

    #[test]
    fn rejects_oversell() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        let orders = [order(1, 1, Buy, dec!(50), dec!(10), dec!(0), 5), order(2, 1, Sell, dec!(100), dec!(10), dec!(0), 6)];
        let err = build_portfolio(&assets, &orders).unwrap_err();
        assert_eq!(err.message, "Venda de 100 PETR4 em 06/09/2026 excede a posição de 50");
    }

    #[test]
    fn chronology_not_insertion_order() {
        // Venda cadastrada antes (id menor), mas com data posterior à compra: válido.
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        let orders = [order(1, 1, Sell, dec!(10), dec!(12), dec!(0), 20), order(2, 1, Buy, dec!(10), dec!(10), dec!(0), 1)];
        assert_eq!(build_portfolio(&assets, &orders).unwrap().sales[0].result, Money(dec!(20)));
    }

    #[test]
    fn fractional_quantities_only_for_fixed_income() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP"), asset(2, "TESOURO2029", AssetType::FixedIncome, "XP")];
        assert!(build_portfolio(&assets, &[order(1, 1, Buy, dec!(1.5), dec!(10), dec!(0), 1)]).is_err());
        assert!(build_portfolio(&assets, &[order(1, 2, Buy, dec!(1.5), dec!(10), dec!(0), 1)]).is_ok());
    }

    #[test]
    fn rejects_same_ticker_with_different_types() {
        let assets = [asset(1, "XPTO11", AssetType::Fii, "XP"), asset(2, "XPTO11", AssetType::Stock, "Rico")];
        assert_eq!(build_portfolio(&assets, &[]).unwrap_err().field, "asset_type");
    }

    #[test]
    fn allocation_by_type() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP"), asset(2, "HGLG11", AssetType::Fii, "XP")];
        let orders = [order(1, 1, Buy, dec!(100), dec!(30), dec!(0), 1), order(2, 2, Buy, dec!(10), dec!(100), dec!(0), 1)];
        let p = build_portfolio(&assets, &orders).unwrap();
        assert_eq!(p.total_cost, Money(dec!(4000)));
        let slices: Vec<_> = p.allocation.iter().map(|s| (s.asset_type, s.percent)).collect();
        assert_eq!(slices, [(AssetType::Stock, dec!(75)), (AssetType::Fii, dec!(25))]);
    }
}
