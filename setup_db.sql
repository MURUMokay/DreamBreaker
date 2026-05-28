-- =====================================================================
-- DreamBreaker — одноразовая настройка БД.
--
-- Запускать ОДИН РАЗ под суперпользователем postgres:
--   psql -U postgres -f setup_db.sql
--
-- Этот скрипт создаёт пользователя, базу и расширения.
-- Сами таблицы создаются автоматически миграциями при запуске приложения.
-- =====================================================================

-- 1. Пользователь приложения (пропускаем, если уже есть).
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'dreambreaker') THEN
        CREATE ROLE dreambreaker LOGIN PASSWORD 'secret';
    END IF;
END
$$;

-- 2. База данных (CREATE DATABASE нельзя в DO-блоке, поэтому через \gexec).
SELECT 'CREATE DATABASE dreambreaker OWNER dreambreaker'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'dreambreaker')
\gexec

-- 3. Подключаемся к новой БД и ставим расширения (нужны права суперпользователя).
\c dreambreaker
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
-- Переключаемся на базу dreambreaker для выдачи прав
\c dreambreaker

-- Даём пользовател dreambreaker права на схему public
GRANT ALL ON SCHEMA public TO dreambreaker;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO dreambreaker;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO dreambreaker;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON FUNCTIONS TO dreambreaker;

\echo 'Готово! Пользователь, база и расширения настроены.'
\echo 'Теперь запусти: cargo run — миграции применятся автоматически.'
