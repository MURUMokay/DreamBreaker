-- Сущность ER: "Игровое поле" (ID поля, ID игры, Тип).
-- Связи "Расположена на" с Собственностью и Магазином оформляются
-- позже (миграция 9), когда обе таблицы уже существуют.
CREATE TABLE game_cells (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    cell_index INT NOT NULL,
    cell_type VARCHAR(20) NOT NULL,                 -- empty, property, shop, start, tax
    tax_amount BIGINT DEFAULT 0,
    UNIQUE(game_id, cell_index),
    CHECK (cell_type IN ('empty', 'property', 'shop', 'start', 'tax'))
);
