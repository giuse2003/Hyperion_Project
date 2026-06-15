@echo off
chcp 65001 > nul
title Hyperion Rust Worker - Distributed BTC Search
echo ===================================================
echo   Hyperion Rust Worker - Client di Ricerca Chiavi  
echo ===================================================
echo.

:: 1. Rileva eseguibile del worker
set EXE_PATH=
if exist "hyperion_worker.exe" (
    set EXE_PATH=hyperion_worker.exe
) else if exist "..\worker\target\release\hyperion_worker.exe" (
    set EXE_PATH=..\worker\target\release\hyperion_worker.exe
)

if "%EXE_PATH%"=="" (
    echo [ERRORE] Eseguibile hyperion_worker.exe non trovato!
    echo.
    echo Per farlo funzionare:
    echo 1. Se sei sul PC principale, compilalo con: cd ..\worker ^&^& cargo build --release
    echo 2. Se sei su un PC esterno, copia l'eseguibile "hyperion_worker.exe" in questa cartella.
    echo.
    pause
    exit /b 1
)

:: 2. Rileva Bloom Filter
set FILTER_PATH=
if exist "filter.bin" (
    set FILTER_PATH=filter.bin
) else if exist "..\worker\target\release\filter.bin" (
    set FILTER_PATH=..\worker\target\release\filter.bin
) else if exist "..\generator\filter.bin" (
    set FILTER_PATH=..\generator\filter.bin
)

if "%FILTER_PATH%"=="" (
    echo [ERRORE] File filter.bin non trovato!
    echo.
    echo Per farlo funzionare:
    echo 1. Genera il filtro usando generator.py nella cartella "generator".
    echo 2. Posiziona il file "filter.bin" in questa cartella.
    echo.
    pause
    exit /b 1
)

:: 3. Copia file necessari in locale se non presenti
if not exist "filter.bin" (
    echo Copia del Bloom Filter [%FILTER_PATH%] in corso...
    copy "%FILTER_PATH%" "filter.bin" > nul
)

if not exist "hyperion_worker.exe" (
    echo Copia del Worker [%EXE_PATH%] in corso...
    copy "%EXE_PATH%" "hyperion_worker.exe" > nul
    set EXE_PATH=hyperion_worker.exe
)

:: 4. Prompt interattivo
set SERVER=127.0.0.1
set /p SERVER="Inserisci l'IP o Hostname del Coordinator [default: 127.0.0.1]: "

set THREADS=4
set /p THREADS="Inserisci il numero di thread CPU da usare [default: 4]: "

set WORKER_ID=
set /p WORKER_ID="Inserisci l'ID di questo Worker (invio per usare il nome PC): "

echo.
echo Avvio di %EXE_PATH%...
echo Server: %SERVER%:8085
echo Thread: %THREADS%

if "%WORKER_ID%"=="" (
    echo Worker ID: %COMPUTERNAME%
    hyperion_worker.exe --server %SERVER% --port 8085 --threads %THREADS%
) else (
    echo Worker ID: %WORKER_ID%
    hyperion_worker.exe --server %SERVER% --port 8085 --threads %THREADS% --worker-id %WORKER_ID%
)

echo.
pause
