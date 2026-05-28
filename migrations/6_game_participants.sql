-- Сущность ER: "Участник игры".
-- Связь "Является" с Игроком (1 игрок — M участий в разных играх).
CREATE TABLE game_participants (
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    position INT NOT NULL DEFAULT 0,                -- Текущая позиция
    balance BIGINT NOT NULL DEFAULT 1000,           -- Баланс (ТЗ п.2.3.6)
    moves_made INT NOT NULL DEFAULT 0,              -- Сделано ходов
    total_spent BIGINT NOT NULL DEFAULT 0,          -- Потрачено
    total_earned BIGINT NOT NULL DEFAULT 0,         -- Получено
    turn_order INT NOT NULL,
    PRIMARY KEY (game_id, user_id)
);
