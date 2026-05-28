-- Сущность ER: "Правила игры" (ID игры PK/FK, Стартовый баланс, Максимум ходов, Целевой баланс).
-- Связь "Следует" 1:1 с games: одна игра — один набор правил.
-- game_id одновременно первичный и внешний ключ, что и даёт связь один-к-одному.
CREATE TABLE game_rules (
    game_id UUID PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    starting_balance BIGINT NOT NULL DEFAULT 1000,  -- Стартовый баланс
    max_turns INT NOT NULL DEFAULT 50,             -- Максимум ходов
    target_balance BIGINT NOT NULL DEFAULT 2500     -- Целевой баланс
);
