-- IRRF ("dedo-duro") retido por mês, informado pelo usuário a partir das notas de corretagem.
-- Abate o IR devido no mês; a sobra vira crédito no mesmo ano (ver shared::tax).
CREATE TABLE monthly_irrf (
    year    INTEGER NOT NULL CHECK (year BETWEEN 1900 AND 9999),
    month   INTEGER NOT NULL CHECK (month BETWEEN 1 AND 12),
    amount  TEXT NOT NULL,
    PRIMARY KEY (year, month)
);
