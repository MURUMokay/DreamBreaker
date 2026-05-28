@echo off
chcp 1251 >nul
set "PGCLIENTENCODING=WIN1251"

REM ============================================================
REM  DreamBreaker - сброс базы данных (удаляет ВСЕ данные!)
REM ============================================================
REM  Полностью удаляет и пересоздаёт БД, применяет setup_db.sql.
REM  Требуется установленный PostgreSQL (команда psql в PATH).
REM ============================================================
echo ============================================================
echo  Сброс базы данных DreamBreaker
echo ============================================================
echo.
echo ВНИМАНИЕ: Это действие безвозвратно удалит ВСЕ данные из базы!
echo           Убедись, что ты действительно хочешь продолжить.
echo.
pause

REM Устанавливаем пароль для автоматической аутентификации
set "PGPASSWORD=postgres"

REM 1. Завершаем активные подключения (чтобы DROP не упал с ошибкой)
echo Завершение фоновых подключений к БД...
psql -U postgres -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='dreambreaker';" >nul 2>&1

REM 2. Удаляем и пересоздаём базу данных.
echo Удаление старой базы данных...
psql -U postgres -c "DROP DATABASE IF EXISTS dreambreaker;"
if %ERRORLEVEL% neq 0 (
    echo.
    echo !!! Ошибка при удалении базы данных.
    echo     Проверь, что PostgreSQL запущен, и повтори попытку.
    set "PGPASSWORD="
    pause >nul
    exit /b 1
)

echo Создание новой базы данных...
psql -U postgres -c "CREATE DATABASE dreambreaker;"
if %ERRORLEVEL% neq 0 (
    echo.
    echo !!! Ошибка при создании базы данных.
    set "PGPASSWORD="
    pause >nul
    exit /b 1
)

REM 3. Применяем первоначальную настройку (пользователь, расширения).
echo.
echo Применение настроек из setup_db.sql...
psql -U postgres -f setup_db.sql
if %ERRORLEVEL% neq 0 (
    echo.
    echo !!! Ошибка при настройке БД.
    echo     Проверь наличие файла setup_db.sql и доступ к PostgreSQL.
    set "PGPASSWORD="
    pause >nul
    exit /b 1
)

REM Очищаем переменную пароля
set "PGPASSWORD="

echo.
echo ============================================================
echo  База данных успешно сброшена и настроена заново!
echo  Теперь запусти игру: run-fast.bat
echo ============================================================
pause >nul