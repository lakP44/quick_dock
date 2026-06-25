@echo off
setlocal
cargo build --release
if errorlevel 1 (
    echo Build failed.
    exit /b 1
)
if exist env (
    if not exist target\release\env mkdir target\release\env
    for %%F in (env\*) do (
        if not exist "target\release\env\%%~nxF" (
            copy /Y "%%F" "target\release\env\%%~nxF" >nul
        )
    )
)
echo.
echo Build completed: target\release\quick_dock.exe
endlocal
