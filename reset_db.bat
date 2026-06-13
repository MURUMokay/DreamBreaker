@echo off
chcp 1251 >nul
set "PGCLIENTENCODING=WIN1251"

REM ============================================================
REM  DreamBreaker - ����� ���� ������ (������� ��� ������!)
REM ============================================================
REM  ��������� ������� � ���������� ��, ��������� setup_db.sql.
REM  ��������� ������������� PostgreSQL (������� psql � PATH).
REM ============================================================
echo ============================================================
echo  ����� ���� ������ DreamBreaker
echo ============================================================
echo.
echo ��������: ��� �������� ������������ ������ ��� ������ �� ����!
echo           �������, ��� �� ������������� ������ ����������.
echo.
pause

REM ������������� ������ ��� �������������� ��������������
set "PGPASSWORD=Commander12s!"

REM 1. ��������� �������� ����������� (����� DROP �� ���� � �������)
echo ���������� ������� ����������� � ��...
psql -U postgres -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='dreambreaker';" >nul 2>&1

REM 2. ������� � ���������� ���� ������.
echo �������� ������ ���� ������...
psql -U postgres -c "DROP DATABASE IF EXISTS dreambreaker;"
if %ERRORLEVEL% neq 0 (
    echo.
    echo !!! ������ ��� �������� ���� ������.
    echo     �������, ��� PostgreSQL �������, � ������� �������.
    set "PGPASSWORD="
    pause >nul
    exit /b 1
)

echo �������� ����� ���� ������...
psql -U postgres -c "CREATE DATABASE dreambreaker;"
if %ERRORLEVEL% neq 0 (
    echo.
    echo !!! ������ ��� �������� ���� ������.
    set "PGPASSWORD="
    pause >nul
    exit /b 1
)

REM 3. ��������� �������������� ��������� (������������, ����������).
echo.
echo ���������� �������� �� setup_db.sql...
psql -U postgres -f setup_db.sql
if %ERRORLEVEL% neq 0 (
    echo.
    echo !!! ������ ��� ��������� ��.
    echo     ������� ������� ����� setup_db.sql � ������ � PostgreSQL.
    set "PGPASSWORD="
    pause >nul
    exit /b 1
)

REM ������� ���������� ������
set "PGPASSWORD="

echo.
echo ============================================================
echo  ���� ������ ������� �������� � ��������� ������!
echo  ������ ������� ����: run-fast.bat
echo ============================================================
pause >nul