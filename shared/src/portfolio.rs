//! Motor de posição: percorre as ordens dia a dia e calcula, por ticker, quantidade, preço
//! médio (PM) e custo; e, para cada venda, o resultado realizado (swing trade ou day trade).
//!
//! Regras (Receita Federal, renda variável):
//! * O PM é consolidado por TICKER, somando todas as corretoras.
//! * Compra: PM = ((Qtd ant. × PM ant.) + (Qtd nova × Preço + Taxas)) / (Qtd ant. + Qtd nova).
//!   Implementado acumulando o custo total (`custo += qtd × preço + taxas`, `PM = custo / qtd`),
//!   que é algebricamente a mesma fórmula e evita arredondar o PM a cada passo.
//! * Venda comum (swing): o PM não muda; resultado = qtd × preço − taxas − qtd × PM.
//! * **Day trade:** compra e venda do mesmo ativo na MESMA corretora no MESMO dia. A quantidade
//!   de day trade é `min(comprado no dia, vendido no dia)`; seu custo é o preço médio das
//!   compras do dia (com taxas), não o PM da carteira. As sobras seguem como swing: compra
//!   excedente entra na posição; venda excedente sai da posição anterior pelo PM.
//! * Ordens só têm data (sem hora). Num mesmo dia, primeiro se casa o day trade de cada
//!   corretora, depois as compras restantes entram na posição e, por fim, as vendas restantes
//!   saem dela. Assim uma compra numa corretora cobre uma venda na outra no mesmo dia.
//! * As taxas de uma venda dividida entre day trade e swing são rateadas pela quantidade.
//!
//! Nada aqui arredonda: o arredondamento para centavos é feito só na exibição/apuração.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::format::{format_date_br, format_quantity};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaleKind {
    /// Operação comum: custo pelo PM da carteira.
    Swing,
    /// Compra e venda no mesmo dia e corretora: custo pelas compras do dia.
    DayTrade,
}

/// Resultado realizado de (parte de) uma ordem de venda. Uma venda que é parcialmente day
/// trade gera dois registros com o mesmo `order_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaleResult {
    pub order_id: i64,
    pub ticker: String,
    pub asset_type: AssetType,
    pub date: NaiveDate,
    pub kind: SaleKind,
    pub quantity: Quantity,
    /// Quantidade × preço de venda (base do limite de isenção de R$ 20 mil, no swing).
    pub gross: Money,
    /// Parcela das taxas da ordem proporcional a `quantity`.
    pub fees: Money,
    /// Quantidade × PM (swing) ou × custo médio das compras do dia (day trade).
    pub cost: Money,
    /// gross − fees − cost. Negativo = prejuízo.
    pub result: Money,
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

impl Portfolio {
    /// Resultado total de uma ordem de venda (somando as partes swing e day trade).
    pub fn order_result(&self, order_id: i64) -> Option<Money> {
        let parts: Vec<_> = self.sales.iter().filter(|s| s.order_id == order_id).collect();
        (!parts.is_empty()).then(|| Money(parts.iter().map(|s| s.result.0).sum()))
    }
}

#[derive(Default)]
struct Book {
    quantity: Decimal,
    cost: Decimal,
    brokers: BTreeSet<String>,
}

/// Parte de uma ordem de venda ainda não casada, a ser baixada da posição (swing).
struct PendingSell<'a> {
    order: &'a Order,
    asset: &'a Asset,
    quantity: Decimal,
    fees: Decimal,
}

fn oversell(asset: &Asset, date: NaiveDate, qty: Decimal, position: Decimal) -> ValidationError {
    ValidationError::new(
        "quantity",
        &format!(
            "Venda de {} {} em {} excede a posição de {}",
            format_quantity(qty),
            asset.ticker,
            format_date_br(date),
            format_quantity(position),
        ),
    )
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
                    &format!("{} já está cadastrado como {} em outra corretora", asset.ticker, previous.label_pt()),
                ));
            }
            _ => {}
        }
    }

    let assets_by_id: HashMap<i64, &Asset> = assets.iter().map(|a| (a.id, a)).collect();
    let asset_of = |order: &Order| {
        assets_by_id.get(&order.asset_id).copied().ok_or_else(|| {
            ValidationError::new("asset_id", &format!("Ordem #{} aponta para um ativo inexistente", order.id))
        })
    };

    // Dia -> ativo (ticker + corretora) -> ordens do dia em ordem de cadastro.
    let mut days: BTreeMap<NaiveDate, BTreeMap<i64, Vec<&Order>>> = BTreeMap::new();
    for order in orders {
        let asset = asset_of(order)?;
        if asset.asset_type != AssetType::FixedIncome && !order.quantity.fract().is_zero() {
            return Err(ValidationError::new(
                "quantity",
                &format!("{} ({}) só aceita quantidades inteiras", asset.ticker, asset.asset_type.label_pt()),
            ));
        }
        days.entry(order.date).or_default().entry(order.asset_id).or_default().push(order);
    }

    let mut books: BTreeMap<&str, Book> = BTreeMap::new();
    let mut sales = Vec::new();

    for (date, by_asset) in days {
        let mut pending_sells: Vec<PendingSell> = Vec::new();

        for (_, mut day_orders) in by_asset {
            day_orders.sort_by_key(|o| o.id);
            let asset = asset_of(day_orders[0])?;
            let book = books.entry(&asset.ticker).or_default();
            book.brokers.insert(asset.broker.clone());

            let buys: Vec<&Order> = day_orders.iter().copied().filter(|o| o.kind == OrderKind::Buy).collect();
            let bought: Decimal = buys.iter().map(|o| o.quantity.0).sum();
            let buy_cost: Decimal = buys.iter().map(|o| o.quantity.0 * o.price.0 + o.fees.0).sum();
            let sold: Decimal = day_orders.iter().filter(|o| o.kind == OrderKind::Sell).map(|o| o.quantity.0).sum();

            // Casa o day trade nas vendas, em ordem de cadastro.
            let day_trade_qty = bought.min(sold);
            let mut to_match = day_trade_qty;
            for sell in day_orders.iter().copied().filter(|o| o.kind == OrderKind::Sell) {
                let qty = sell.quantity.0;
                let matched = to_match.min(qty);
                to_match -= matched;
                // Rateio das taxas; a parte swing recebe o restante (a soma fecha exata).
                let dt_fees = if matched == qty { sell.fees.0 } else { sell.fees.0 * matched / qty };
                if matched > Decimal::ZERO {
                    let cost = buy_cost * matched / bought;
                    let gross = matched * sell.price.0;
                    sales.push(SaleResult {
                        order_id: sell.id,
                        ticker: asset.ticker.clone(),
                        asset_type: asset.asset_type,
                        date,
                        kind: SaleKind::DayTrade,
                        quantity: Quantity(matched),
                        gross: Money(gross),
                        fees: Money(dt_fees),
                        cost: Money(cost),
                        result: Money(gross - dt_fees - cost),
                    });
                }
                if matched < qty {
                    pending_sells.push(PendingSell { order: sell, asset, quantity: qty - matched, fees: sell.fees.0 - dt_fees });
                }
            }

            // Compras excedentes entram na posição com sua parte do custo.
            let remaining = bought - day_trade_qty;
            if remaining > Decimal::ZERO {
                book.cost += if day_trade_qty.is_zero() { buy_cost } else { buy_cost * remaining / bought };
                book.quantity += remaining;
            }
        }

        // Vendas comuns saem da posição (já incluindo as compras do dia), pelo PM.
        for sell in pending_sells {
            let book = books.get_mut(sell.asset.ticker.as_str()).expect("livro criado acima");
            if sell.quantity > book.quantity {
                return Err(oversell(sell.asset, date, sell.quantity, book.quantity));
            }
            let cost = if sell.quantity == book.quantity { book.cost } else { book.cost * sell.quantity / book.quantity };
            let gross = sell.quantity * sell.order.price.0;
            sales.push(SaleResult {
                order_id: sell.order.id,
                ticker: sell.asset.ticker.clone(),
                asset_type: sell.asset.asset_type,
                date,
                kind: SaleKind::Swing,
                quantity: Quantity(sell.quantity),
                gross: Money(gross),
                fees: Money(sell.fees),
                cost: Money(cost),
                result: Money(gross - sell.fees - cost),
            });
            book.cost -= cost;
            book.quantity -= sell.quantity;
            if book.quantity.is_zero() {
                book.cost = Decimal::ZERO;
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
    fn same_day_same_broker_is_day_trade_with_own_cost() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        let orders = [
            order(1, 1, Buy, dec!(100), dec!(10), dec!(0), 1), // posição anterior, PM 10
            order(2, 1, Buy, dec!(50), dec!(20), dec!(10), 2), // dia 2: custo do dia 20,20/un.
            order(3, 1, Sell, dec!(50), dec!(21), dec!(5), 2),
        ];
        let p = build_portfolio(&assets, &orders).unwrap();
        assert_eq!(p.sales.len(), 1);
        let dt = &p.sales[0];
        assert_eq!(dt.kind, SaleKind::DayTrade);
        assert_eq!(dt.cost, Money(dec!(1010))); // 50 × 20 + 10, e não 50 × PM
        assert_eq!(dt.result, Money(dec!(35))); // 1.050 − 5 − 1.010
        // A posição anterior fica intacta.
        assert_eq!(p.positions[0].quantity, Quantity(dec!(100)));
        assert_eq!(p.positions[0].average_price, Money(dec!(10)));
    }

    #[test]
    fn day_trade_leftovers_go_to_swing() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        // Compra 100, vende 30 no mesmo dia: 30 day trade, 70 entram na posição.
        let orders = [order(1, 1, Buy, dec!(100), dec!(10), dec!(10), 1), order(2, 1, Sell, dec!(30), dec!(12), dec!(0), 1)];
        let p = build_portfolio(&assets, &orders).unwrap();
        assert_eq!(p.sales[0].cost, Money(dec!(303))); // 30 × 10,10
        assert_eq!(p.positions[0].quantity, Quantity(dec!(70)));
        assert_eq!(p.positions[0].total_cost, Money(dec!(707))); // 70 × 10,10

        // Posição de 100 a PM 10; no dia compra 20 e vende 50: 20 day trade + 30 swing.
        let orders = [
            order(1, 1, Buy, dec!(100), dec!(10), dec!(0), 1),
            order(2, 1, Buy, dec!(20), dec!(15), dec!(0), 2),
            order(3, 1, Sell, dec!(50), dec!(16), dec!(10), 2),
        ];
        let p = build_portfolio(&assets, &orders).unwrap();
        let parts: Vec<_> = p.sales.iter().map(|s| (s.kind, s.quantity.0, s.fees.0, s.result.0)).collect();
        assert_eq!(
            parts,
            [
                (SaleKind::DayTrade, dec!(20), dec!(4), dec!(16)), // 320 − 4 − 300
                (SaleKind::Swing, dec!(30), dec!(6), dec!(174)),   // 480 − 6 − 300
            ]
        );
        assert_eq!(p.order_result(3), Some(Money(dec!(190))));
        assert_eq!(p.positions[0].quantity, Quantity(dec!(70)));
        assert_eq!(p.positions[0].average_price, Money(dec!(10)));
    }

    #[test]
    fn different_brokers_same_day_is_not_day_trade() {
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP"), asset(2, "PETR4", AssetType::Stock, "Rico")];
        let orders = [
            order(1, 1, Buy, dec!(100), dec!(10), dec!(0), 1),
            order(2, 2, Buy, dec!(100), dec!(20), dec!(0), 2),
            order(3, 1, Sell, dec!(100), dec!(25), dec!(0), 2),
        ];
        let p = build_portfolio(&assets, &orders).unwrap();
        assert_eq!(p.sales[0].kind, SaleKind::Swing);
        // A compra do dia (outra corretora) entra no PM antes da venda: PM 15.
        assert_eq!(p.sales[0].cost, Money(dec!(1500)));
    }

    #[test]
    fn same_day_buy_covers_sell_without_prior_position() {
        // Antes ordens do mesmo dia seguiam o id: venda cadastrada antes da compra falhava.
        let assets = [asset(1, "PETR4", AssetType::Stock, "XP")];
        let orders = [order(1, 1, Sell, dec!(10), dec!(12), dec!(0), 5), order(2, 1, Buy, dec!(10), dec!(10), dec!(0), 5)];
        let p = build_portfolio(&assets, &orders).unwrap();
        assert_eq!((p.sales[0].kind, p.sales[0].result), (SaleKind::DayTrade, Money(dec!(20))));
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
