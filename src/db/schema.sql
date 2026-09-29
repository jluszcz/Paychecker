CREATE TABLE field (
    id       INTEGER PRIMARY KEY,
    name     TEXT    NOT NULL UNIQUE,
    kind     TEXT    NOT NULL CHECK (kind IN ('income', 'deduction')),
    position INTEGER NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1))
);

CREATE TABLE paycheck (
    id   INTEGER PRIMARY KEY,
    date TEXT    NOT NULL UNIQUE  -- ISO YYYY-MM-DD
);

CREATE TABLE amount (
    paycheck_id INTEGER NOT NULL REFERENCES paycheck(id) ON DELETE CASCADE,
    field_id    INTEGER NOT NULL REFERENCES field(id),
    cents       INTEGER NOT NULL,
    PRIMARY KEY (paycheck_id, field_id)
);
