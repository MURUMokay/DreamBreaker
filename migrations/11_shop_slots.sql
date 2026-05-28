-- Сущность ER: "Слот магазина" (ID слота, ID усиления FK, ID магазина FK, Номер слота, Статус).
-- Связь "Состоит из": магазин (1) состоит из слотов (M).
-- Связь "Содержится в": усиление (1) содержится в слотах (M).
CREATE TABLE shop_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,    -- ID магазина
    power_up_id UUID REFERENCES power_ups(id) ON DELETE SET NULL,    -- ID усиления
    slot_index INT NOT NULL,                        -- Номер слота
    status VARCHAR(20) NOT NULL DEFAULT 'available', -- Статус: available, sold, locked
    cost BIGINT NOT NULL,
    UNIQUE (shop_id, slot_index),
    CHECK (status IN ('available', 'sold', 'locked'))
);
