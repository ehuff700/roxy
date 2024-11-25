@echo off
setlocal enabledelayedexpansion

REM Navigate to the certs directory
cd /d "%~dp0src\core\certs" || (
    echo [ERROR] Failed to navigate to directory: src\core\certs
    exit /b 1
)

REM Delete the existing certificates
for %%f in (*.cer *.key) do (
    del "%%f" || (
        echo [ERROR] Failed to delete "%%f"
        exit /b 1
    )
    echo [INFO] Deleted "%%f"
)

REM Generate the private key
openssl genrsa -out roxy.key 4096
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Failed to generate private key.
    exit /b 1
)

REM Generate a self-signed certificate
openssl req -x509 -new -nodes -key roxy.key -sha512 -days 3650 -out roxy.cer -subj "/CN=Roxy"
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Failed to generate self-signed certificate.
    exit /b 1
)

echo [INFO] Certificate and private key successfully generated.
exit /b 0
