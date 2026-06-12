INSERT INTO users (username, type, password_hash, is_active)
VALUES
    ('Бот Алекс',  'bot', 'bot', TRUE),
    ('Бот Нова',   'bot', 'bot', TRUE),
    ('Бот Зара',   'bot', 'bot', TRUE),
    ('Бот Кайрос', 'bot', 'bot', TRUE)
ON CONFLICT (username) DO NOTHING;
