@echo off
chcp 65001 > nul
title Hyperion Rust Worker - Distributed BTC Search
echo ===================================================
echo   Hyperion Rust Worker - Client di Ricerca Chiavi  
echo ===================================================
echo.

:: Determina la posizione dell'eseguibile
set EXE_PATH=hyperion_worker.exe
if not exist "%EXE_PATH%" (
    set EXE_PATH=worker\target\release\hyperion_worker.exe
)

if not exist "%EXE_PATH%" (
    echo [ERRORE] Eseguibile hyperion_worker.exe non trovato!
    echo Assicurati di aver compilato il progetto con: cargo build --release
    echo oppure di aver copiato l'eseguibile in questa cartella.
    echo.
    pause
    exit /b 1
)

:: Determina la posizione di filter.bin
set FILTER_PATH=filter.bin
if not exist "%FILTER_PATH%" (
    if exist "worker\target\release\filter.bin" (
        set FILTER_PATH=worker\target\release\filter.bin
    ) else if exist "generator\filter.bin" (
        set FILTER_PATH=generator\filter.bin
    )
)

if not exist "%FILTER_PATH%" (
    echo [ERRORE] File filter.bin non trovato!
    echo Genera il filtro usando generator.py e posizionalo in questa cartella.
    echo.
    pause
    exit /b 1
)

:: Se il filtro si trova altrove, copialo qui per permettere al worker di leggerlo localmente
if not exist "filter.bin" (
    echo Copia di %FILTER_PATH% nella cartella corrente...
    copy "%FILTER_PATH%" "filter.bin" > nul
)

set /p SERVER="Inserisci l'IP o Hostname del Coordinator [default: 127.0.0.1]: "
if "%SERVER%"=="" set SERVER=127.0.0.1

set /p THREADS="Inserisci il numero di thread CPU da usare [default: 4]: "
if "%THREADS%"=="" set THREADS=4

set /p WORKER_ID="Inserisci l'ID di questo Worker (invio per usare il nome PC): "

echo.
echo Avvio di %EXE_PATH%...
echo Server: %SERVER%:8085
echo Thread: %THREADS%
if not "%WORKER_ID%"=="" (
    echo Worker ID: %WORKER_ID%
    "%EXE_PATH%" --server %SERVER% --port 8085 --threads %THREADS% --worker-id %WORKER_ID%
) else (
    echo Worker ID: %COMPUTERNAME%
    "%EXE_PATH%" --server %SERVER% --port 8085 --threads %THREADS%
)

echo.
pause
