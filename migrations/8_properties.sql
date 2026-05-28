-- Сущность ER: "Собственность" (ID собственности, ID поля FK, ID участника игры FK,
-- Название, Стоимость покупки, Стоимость аренды, Усиления).
-- Связь "Расположена на" с полем (1:1) и "Имеет" с участником (владелец).
CREATE TABLE properties (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cell_id UUID REFERENCES game_cells(id) ON DELETE CASCADE,  -- ID поля (где расположена)
    -- Владелец: участник игры. NULL = собственность ничья (не куплена).
    owner_game_id UUID,
    owner_user_id UUID,
    name VARCHAR(100) NOT NULL,                     -- Название
    purchase_cost BIGINT NOT NULL,                  -- Стоимость покупки
    rent_cost BIGINT NOT NULL,                      -- Стоимость аренды
    upgrades JSONB DEFAULT '[]'::JSONB,             -- Усиления (ТЗ п.2.3.4)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (owner_game_id, owner_user_id)
        REFERENCES game_participants(game_id, user_id) ON DELETE SET NULL
);
