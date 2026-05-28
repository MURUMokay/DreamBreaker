-- Сущность ER: "Усиление" (ID усиления, Название, Описание, Стоимость, Эффект).
CREATE TABLE power_ups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,                     -- Название
    description TEXT,                               -- Описание
    cost BIGINT NOT NULL,                           -- Стоимость
    effect JSONB NOT NULL                           -- Эффект, напр. { "type": "shield", "duration": 2 }
);
