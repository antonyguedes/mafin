-- Esquema inicial do Mafin.
--
-- Convenções:
-- * Valores decimais (dinheiro/quantidade) são TEXT com o Decimal em forma canônica
--   ("1234.5"). NÃO use NUMERIC/REAL: o SQLite converteria para float.
--   NÃO use SUM()/AVG() nessas colunas; agregue em Rust com rust_decimal.
-- * Datas são TEXT ISO-8601 "YYYY-MM-DD"; o CHECK garante o formato canônico,
--   o que torna comparações de string (>=, <) equivalentes a comparações de data.
-- * Enums são TEXT em snake_case, iguais ao serde/sqlx do pacote `shared`.

CREATE TABLE transactions (
    id          INTEGER PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('income', 'expense')),
    amount      TEXT NOT NULL,
    date        TEXT NOT NULL CHECK (date IS date(date)),
    category    TEXT NOT NULL CHECK (length(category) > 0),
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_transactions_date ON transactions (date);

CREATE TABLE assets (
    id          INTEGER PRIMARY KEY,
    ticker      TEXT NOT NULL CHECK (length(ticker) > 0),
    asset_type  TEXT NOT NULL CHECK (asset_type IN ('stock', 'fii', 'fixed_income')),
    broker      TEXT NOT NULL CHECK (length(broker) > 0),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (ticker, broker)
);

CREATE TABLE orders (
    id          INTEGER PRIMARY KEY,
    -- NO ACTION (padrão): não é possível excluir um ativo que possui ordens.
    -- Evite `ON DELETE RESTRICT`: o SQLite o reporta como SQLITE_CONSTRAINT_TRIGGER,
    -- que o sqlx não reconhece como violação de chave estrangeira.
    asset_id    INTEGER NOT NULL REFERENCES assets (id),
    kind        TEXT NOT NULL CHECK (kind IN ('buy', 'sell')),
    quantity    TEXT NOT NULL,
    price       TEXT NOT NULL,
    fees        TEXT NOT NULL DEFAULT '0',
    date        TEXT NOT NULL CHECK (date IS date(date)),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Ordem cronológica por ativo: base do cálculo de preço médio.
CREATE INDEX idx_orders_asset_date ON orders (asset_id, date, id);
CREATE INDEX idx_orders_date ON orders (date);
