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
    set EXE_PATH=..\worker\target\release\hyperion_worker.exe
)

if not exist "%EXE_PATH%" (
    echo [ERRORE] Eseguibile hyperion_worker.exe non trovato!
    echo.
    echo Per farlo funzionare:
    echo 1. Se sei sul PC principale, assicurati di aver compilato il progetto con:
    echo    cd ..\worker && cargo build --release
    echo 2. Se sei su un PC esterno, copia l'eseguibile "hyperion_worker.exe"
    echo    (che trovi in "worker\target\release\" sul PC principale) in questa cartella.
    echo.
    pause
    exit /b 1
)

:: Determina la posizione di filter.bin
set FILTER_PATH=filter.bin
if not exist "%FILTER_PATH%" (
    if exist "..\worker\target\release\filter.bin" (
        set FILTER_PATH=..\worker\target\release\filter.bin
    ) else if exist "..\generator\filter.bin" (
        set FILTER_PATH=..\generator\filter.bin
    )
)

if not exist "%FILTER_PATH%" (
    echo [ERRORE] File filter.bin non trovato!
    echo.
    echo Per farlo funzionare:
    echo 1. Genera il filtro usando il generator (nella cartella "generator").
    echo 2. Posiziona il file "filter.bin" in questa cartella.
    echo.
    pause
    exit /b 1
)

:: Se il filtro si trova altrove in locale, copialo qui per permettere al worker di leggerlo localmente
if not exist "filter.bin" (
    echo Copia di %FILTER_PATH% nella cartella corrente...
    copy "%FILTER_PATH%" "filter.bin" > nul
)

:: Se l'eseguibile si trova nella cartella target, copialo qui per renderla stand-alone
if not exist "hyperion_worker.exe" (
    if exist "..\worker\target\release\hyperion_worker.exe" (
        echo Copia di %EXE_PATH% nella cartella corrente...
        copy "..\worker\target\release\hyperion_worker.exe" "hyperion_worker.exe" > nul
        set EXE_PATH=hyperion_worker.exe
    )
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
