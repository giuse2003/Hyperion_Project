@echo off
title Hyperion Worker
cd worker

set "SERVER_IP=127.0.0.1"
set /p "USER_IP=Inserisci IP del Server Coordinator (premi INVIO per usare %SERVER_IP%): "

if not "%USER_IP%"=="" set "SERVER_IP=%USER_IP%"

echo.
echo Avvio del worker collegato a: %SERVER_IP%...
echo.

target\x86_64-pc-windows-gnu\release\hyperion_worker.exe --server %SERVER_IP%
pause
