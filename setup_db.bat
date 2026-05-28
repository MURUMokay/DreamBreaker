@echo off
chcp 1251 >nul
set "PGCLIENTENCODING=WIN1251"

REM ============================================================
REM  DreamBreaker - первоначальная настройка (запустить ОДИН РАЗ)
REM ============================================================
echo ============================================================
echo  Настройка базы данных DreamBreaker
echo ============================================================
echo.
echo Запуск скрипта создания БД от имени суперпользователя postgres...
echo.

REM Устанавливаем пароль для автоматической аутентификации
set "PGPASSWORD=postgres"

REM 1. Создаём пользователя, БД и расширения через setup_db.sql.
psql -U postgres -f setup_db.sql
if %ERRORLEVEL% neq 0 (
    echo.
    echo !!! Ошибка при настройке БД.
    echo     Проверь, что PostgreSQL установлен и команда psql доступна.
    echo     Если psql не найден - добавь папку PostgreSQL\bin в PATH.
    set "PGPASSWORD="
    pause >nul
    exit /b 1
)

REM Очищаем переменную пароля после использования
set "PGPASSWORD="

REM 2. Создаём .env, если его ещё нет.
if not exist .env (
    echo DATABASE_URL=postgres://dreambreaker:secret@localhost:5432/dreambreaker> .env
    echo Файл .env создан.
) else (
    echo.
    echo ВНИМАНИЕ: файл .env уже существует — пропускаю создание.
    echo           Если ты переустанавливал PostgreSQL или менял пароль,
    echo           проверь DATABASE_URL в .env вручную.
)
echo.
echo ============================================================
echo  Настройка завершена!
echo  Теперь запусти игру: run-fast.bat
echo ============================================================
pause >nul