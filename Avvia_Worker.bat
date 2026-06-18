@echo off
title Hyperion Worker
cd worker

set "SERVER_IP=127.0.0.1"
set /p "USER_IP=Inserisci IP del Server Coordinator (premi INVIO per usare %SERVER_IP%): "

if not "%USER_IP%"=="" set "SERVER_IP=%USER_IP%"

echo.
echo Avvio del worker collegato a: %SERVER_IP%...
echo.

if "%SERVER_IP:~0,2%"=="\\" set "SERVER_IP=%SERVER_IP:~2%"

set "BATCH_MILLIONS=8"
set /p "USER_MILLIONS=Quanti milioni di chiavi elaborare per volta? (es. 2, 4, 8, 14) [INVIO per %BATCH_MILLIONS%]: "
if not "%USER_MILLIONS%"=="" set "BATCH_MILLIONS=%USER_MILLIONS%"

set /a "BATCH_SIZE=%BATCH_MILLIONS% * 1048576"

echo.
echo Avvio del worker con %BATCH_MILLIONS% milioni di chiavi per blocco (Batch reale: %BATCH_SIZE%)...

target\release\hyperion_worker.exe --server %SERVER_IP% --batch-size %BATCH_SIZE%
pause
