-- Сущность ER: "Игра" (ID игры, Статус, Дата создания, Дата последнего сохранения, Сид).
-- Правила вынесены в отдельную таблицу game_rules (связь "Следует" 1:1).
CREATE TABLE games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- Статус
    seed BIGINT NOT NULL,                           -- Сид
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),  -- Дата создания
    last_saved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), -- Дата последнего сохранения
    CHECK (status IN ('pending', 'active', 'paused', 'finished', 'surrender'))
);
