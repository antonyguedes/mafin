//! Descobre o ticker de um negócio a partir da "especificação do título" da nota.
//!
//! Notas SINACOR costumam trazer o nome de pregão da empresa e a classe ("PETROBRAS PN N2"),
//! não o ticker. Ordem de tentativa:
//! 1. associação salva numa importação anterior (o usuário já confirmou);
//! 2. um ticker escrito no próprio texto (comum em FIIs: "FII CSHG LOG HGLG11 CI");
//! 3. nome de pregão conhecido + classe (ON → 3, PN → 4, PNA → 5, PNB → 6, UNT → 11).
//!
//! Sugestões (2 e 3) são sempre confirmadas pelo usuário na prévia.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use shared::import::TickerSource;
use shared::{Asset, AssetType};

static TICKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([A-Z]{4}\d{1,2})F?\b").unwrap());

/// Nome de pregão → raiz do ticker. Lista curta das empresas mais negociadas; o resto o
/// usuário informa uma vez e fica salvo como associação.
const COMPANIES: [(&str, &str); 40] = [
    ("PETROBRAS", "PETR"),
    ("VALE", "VALE"),
    ("ITAUUNIBANCO", "ITUB"),
    ("BRADESCO", "BBDC"),
    ("BRASIL", "BBAS"),
    ("AMBEV S/A", "ABEV"),
    ("B3", "B3SA"),
    ("WEG", "WEGE"),
    ("ITAUSA", "ITSA"),
    ("SANTANDER BR", "SANB"),
    ("BTGP BANCO", "BPAC"),
    ("SUZANO S.A.", "SUZB"),
    ("GERDAU", "GGBR"),
    ("GERDAU MET", "GOAU"),
    ("JBS", "JBSS"),
    ("ELETROBRAS", "ELET"),
    ("EQUATORIAL", "EQTL"),
    ("RUMO S.A.", "RAIL"),
    ("LOCALIZA", "RENT"),
    ("BBSEGURIDADE", "BBSE"),
    ("CEMIG", "CMIG"),
    ("COPEL", "CPLE"),
    ("SABESP", "SBSP"),
    ("TAESA", "TAEE"),
    ("KLABIN S/A", "KLBN"),
    ("CIELO", "CIEL"),
    ("MAGAZ LUIZA", "MGLU"),
    ("EMBRAER", "EMBR"),
    ("TIM", "TIMS"),
    ("TELEF BRASIL", "VIVT"),
    ("RAIADROGASIL", "RADL"),
    ("LOJAS RENNER", "LREN"),
    ("PETRORIO", "PRIO"),
    ("PRIO", "PRIO"),
    ("BRASKEM", "BRKM"),
    ("CSN", "CSNA"),
    ("USIMINAS", "USIM"),
    ("TOTVS", "TOTS"),
    ("HAPVIDA", "HAPV"),
    ("CPFL ENERGIA", "CPFE"),
];

/// Classe da ação → sufixo numérico do ticker.
fn class_suffix(token: &str) -> Option<&'static str> {
    Some(match token {
        "ON" => "3",
        "PN" => "4",
        "PNA" => "5",
        "PNB" => "6",
        "UNT" => "11",
        _ => return None,
    })
}

pub fn normalize_spec(spec: &str) -> String {
    spec.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase()
}

fn from_company_name(spec: &str) -> Option<String> {
    let tokens: Vec<&str> = spec.split_whitespace().collect();
    let class_at = tokens.iter().position(|t| class_suffix(t).is_some())?;
    let company = tokens[..class_at].join(" ");
    let root = COMPANIES.iter().find(|(name, _)| *name == company).map(|(_, root)| *root)?;
    Some(format!("{root}{}", class_suffix(tokens[class_at])?))
}

/// Dados do banco usados para resolver: associações salvas e ativos já cadastrados.
#[derive(Default)]
pub struct ResolveContext {
    /// Especificação normalizada → ticker.
    pub aliases: HashMap<String, String>,
    pub assets: Vec<Asset>,
}

impl ResolveContext {
    pub fn resolve(&self, spec: &str) -> (Option<String>, TickerSource, AssetType) {
        let spec = normalize_spec(spec);
        let (ticker, source) = if let Some(ticker) = self.aliases.get(&spec) {
            (Some(ticker.clone()), TickerSource::Alias)
        } else if let Some(c) = TICKER.captures(&spec) {
            (Some(c[1].to_string()), TickerSource::InText)
        } else if let Some(ticker) = from_company_name(&spec) {
            (Some(ticker), TickerSource::CompanyName)
        } else {
            (None, TickerSource::Unknown)
        };
        (ticker.clone(), source, self.asset_type(ticker.as_deref(), &spec))
    }

    /// Tipo do ativo: o cadastrado, se já existe; senão, FII quando a nota indica.
    fn asset_type(&self, ticker: Option<&str>, spec: &str) -> AssetType {
        if let Some(asset) = ticker.and_then(|t| self.assets.iter().find(|a| a.ticker == t)) {
            return asset.asset_type;
        }
        let tokens: Vec<&str> = spec.split_whitespace().collect();
        let fii = tokens.first() == Some(&"FII") || (tokens.contains(&"CI") && ticker.is_some_and(|t| t.ends_with("11")));
        if fii { AssetType::Fii } else { AssetType::Stock }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_order() {
        let mut ctx = ResolveContext::default();
        assert_eq!(ctx.resolve("PETROBRAS PN N2"), (Some("PETR4".into()), TickerSource::CompanyName, AssetType::Stock));
        assert_eq!(ctx.resolve("petrobras   on  n2"), (Some("PETR3".into()), TickerSource::CompanyName, AssetType::Stock));
        assert_eq!(ctx.resolve("FII CSHG LOG HGLG11 CI"), (Some("HGLG11".into()), TickerSource::InText, AssetType::Fii));
        assert_eq!(ctx.resolve("SANTANDER BR UNT N2"), (Some("SANB11".into()), TickerSource::CompanyName, AssetType::Stock));
        assert_eq!(ctx.resolve("GERDAU MET PN N1").0.as_deref(), Some("GOAU4"), "nome mais específico");
        assert_eq!(ctx.resolve("PETR4F"), (Some("PETR4".into()), TickerSource::InText, AssetType::Stock), "fracionário");
        assert_eq!(ctx.resolve("FII XPTO CI").1, TickerSource::Unknown);
        assert_eq!(ctx.resolve("FII XPTO CI").2, AssetType::Fii);

        ctx.aliases.insert("FII XPTO CI".into(), "XPTO11".into());
        assert_eq!(ctx.resolve("FII  xpto CI"), (Some("XPTO11".into()), TickerSource::Alias, AssetType::Fii));
    }

    #[test]
    fn known_asset_type_wins() {
        let ctx = ResolveContext {
            aliases: HashMap::new(),
            assets: vec![Asset { id: 1, ticker: "BOVA11".into(), asset_type: AssetType::Stock, broker: "XP".into() }],
        };
        assert_eq!(ctx.resolve("ISHARES BOVA BOVA11 CI").2, AssetType::Stock);
    }
}
