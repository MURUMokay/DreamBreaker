#!/usr/bin/env bash
# reset_db.sh — Полный сброс БД DreamBreaker
# Удаляет пользователя и базу данных, затем создаёт их заново.
# Читает DATABASE_URL из .env файла в текущей директории.
#
# Использование:
#   ./reset_db.sh
#   ./reset_db.sh --env /path/to/.env
#   ./reset_db.sh --force   # без подтверждения

set -euo pipefail

ENV_FILE=".env"
FORCE=0

# ── Аргументы ─────────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --env)   ENV_FILE="$2"; shift 2 ;;
        --force) FORCE=1; shift ;;
        *) echo "Неизвестный аргумент: $1" >&2; exit 1 ;;
    esac
done

# ── 1. Читаем .env ────────────────────────────────────────────────────────────

if [[ ! -f "$ENV_FILE" ]]; then
    echo "Ошибка: файл '$ENV_FILE' не найден." >&2
    exit 1
fi

DB_URL=$(grep -E '^DATABASE_URL\s*=' "$ENV_FILE" | head -1 | sed 's/^DATABASE_URL\s*=\s*//' | tr -d '"'"'")

if [[ -z "$DB_URL" ]]; then
    echo "Ошибка: DATABASE_URL не найден в '$ENV_FILE'." >&2
    exit 1
fi

# ── 2. Парсим DATABASE_URL ────────────────────────────────────────────────────
# Формат: postgres://user:password@host:port/dbname

if [[ "$DB_URL" =~ ^postgres(ql)?://([^:]+):([^@]+)@([^:/]+)(:([0-9]+))?/(.+)$ ]]; then
    DB_USER="${BASH_REMATCH[2]}"
    DB_PASSWORD="${BASH_REMATCH[3]}"
    DB_HOST="${BASH_REMATCH[4]}"
    DB_PORT="${BASH_REMATCH[6]:-5432}"
    DB_NAME="${BASH_REMATCH[7]}"
else
    echo "Ошибка: не удалось распарсить DATABASE_URL: $DB_URL" >&2
    exit 1
fi

# ── 3. Подтверждение ──────────────────────────────────────────────────────────

echo ""
echo "  СБРОС БАЗЫ ДАННЫХ"
echo "  ─────────────────────────────────────"
echo "  Хост:         $DB_HOST:$DB_PORT"
echo "  База:         $DB_NAME"
echo "  Пользователь: $DB_USER"
echo "  ─────────────────────────────────────"
echo "  Все данные будут УДАЛЕНЫ безвозвратно!"
echo ""

if [[ $FORCE -eq 0 ]]; then
    read -rp "Продолжить? (да/нет): " answer
    case "$answer" in
        да|yes|y|д) ;;
        *) echo "Отменено."; exit 0 ;;
    esac
fi

# ── 4. Передаём пароль через переменную окружения ────────────────────────────

export PGPASSWORD="$DB_PASSWORD"

# ИСПРАВЛЕНИЕ: очищаем PGPASSWORD при любом выходе — и при успехе, и при ошибке
cleanup() {
    unset PGPASSWORD
}
trap cleanup EXIT

psql_run() {
    local database="$1"
    local sql="$2"
    # ИСПРАВЛЕНИЕ: все аргументы в кавычках — защита от спецсимволов в пароле/именах
    psql \
        --host="$DB_HOST" \
        --port="$DB_PORT" \
        --username=postgres \
        --dbname="$database" \
        --no-password \
        --command="$sql"
}

# ── 5. Сброс ─────────────────────────────────────────────────────────────────

echo "[1/4] Отключаем активные соединения с '$DB_NAME'..."
psql_run postgres "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '$DB_NAME' AND pid <> pg_backend_pid();"

echo "[2/4] Удаляем базу данных '$DB_NAME'..."
psql_run postgres "DROP DATABASE IF EXISTS $DB_NAME;"

echo "[3/4] Удаляем пользователя '$DB_USER'..."
psql_run postgres "DROP USER IF EXISTS $DB_USER;"

echo "[4/4] Создаём пользователя и базу данных заново..."
psql_run postgres "CREATE USER $DB_USER WITH PASSWORD '$DB_PASSWORD';"
psql_run postgres "CREATE DATABASE $DB_NAME OWNER $DB_USER;"

# ИСПРАВЛЕНИЕ: восстанавливаем расширения, которые ставит setup_db.sql,
# иначе приложение упадёт при первом запуске после сброса.
echo "[+] Восстанавливаем расширения PostgreSQL..."
psql_run "$DB_NAME" "CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\"; CREATE EXTENSION IF NOT EXISTS pgcrypto;"

# ── 6. Готово ─────────────────────────────────────────────────────────────────

echo ""
echo "  Готово! База данных сброшена."
echo "  Запусти 'cargo run' — миграции применятся автоматически."
echo ""
