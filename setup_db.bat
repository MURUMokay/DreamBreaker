@echo off
chcp 1251 >nul
set "PGCLIENTENCODING=WIN1251"

REM ============================================================
REM  DreamBreaker - �������������� ��������� (��������� ���� ���)
REM ============================================================
echo ============================================================
echo  ��������� ���� ������ DreamBreaker
echo ============================================================
echo.
echo ������ ������� �������� �� �� ����� ����������������� postgres...
echo.

REM ������������� ������ ��� �������������� ��������������
set "PGPASSWORD=Commander12s!"

REM 1. ������ ������������, �� � ���������� ����� setup_db.sql.
psql -U postgres -f setup_db.sql
if %ERRORLEVEL% neq 0 (
    echo.
    echo !!! ������ ��� ��������� ��.
    echo     �������, ��� PostgreSQL ���������� � ������� psql ��������.
    echo     ���� psql �� ������ - ������ ����� PostgreSQL\bin � PATH.
    set "PGPASSWORD="
    pause >nul
    exit /b 1
)

REM ������� ���������� ������ ����� �������������
set "PGPASSWORD="

REM 2. ������ .env, ���� ��� ��� ���.
if not exist .env (
    echo DATABASE_URL=postgres://dreambreaker:secret@localhost:5432/dreambreaker> .env
    echo ���� .env ������.
) else (
    echo.
    echo ��������: ���� .env ��� ���������� � ��������� ��������.
    echo           ���� �� ���������������� PostgreSQL ��� ����� ������,
    echo           ������� DATABASE_URL � .env �������.
)
echo.
echo ============================================================
echo  ��������� ���������!
echo  ������ ������� ����: run-fast.bat
echo ============================================================
pause >nul