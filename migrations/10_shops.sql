-- Сущность ER: "Магазин усилений" (ID магазина, ID поля FK, ID игры FK, Смещение).
-- Связь "Расположена на" с игровым полем.
CREATE TABLE shops (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cell_id UUID REFERENCES game_cells(id) ON DELETE CASCADE,  -- ID поля
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE, -- ID игры
    offset_value INT NOT NULL DEFAULT 0             -- Смещение
);
