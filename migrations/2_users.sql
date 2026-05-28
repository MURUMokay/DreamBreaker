-- Сущность ER: "Игрок" (ID игрока, Имя, Тип, Хэш пароля).
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE,           -- Имя
    type VARCHAR(20) NOT NULL DEFAULT 'human',      -- Тип: human, bot, admin
    password_hash VARCHAR(100) NOT NULL,            -- Хэш пароля (ТЗ п.4.3.4)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    CHECK (type IN ('human', 'bot', 'admin'))
);
