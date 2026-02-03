@echo off

if "%1"=="-i" (
    set FILE=%2
    set OUT_DIR=%3
    goto MAIN
) else (
    set FILE=%1
    set OUT_DIR=%2
)

set FINISHED=Compile Finished. Check %OUT_DIR% for binaries.

:CHECK
    :: Check if Environment Variable Defined.
    if not defined nasm (call warn not_def nasm && set nasm=nasm)
    if not defined slimec (call warn not_def slimec && set slimec=slimec)
    if not defined link (call warn not_def link && set link=golink)
    if not defined link_files (call warn not_def link_files && set link_files=kernel32.dll)

:MAIN
    :: Check if FILE exists, BUILD if yes. Main Function Entry. Needs FILE be set
    if "%FILE%"=="" goto USAGE
    if not exist %FILE% goto FILE_NOT_EXIST
    goto BUILD

:BUILD
    :: BUILD the project
    rd /s /q %OUT_DIR%
    mkdir %OUT_DIR%
    if "%DRY_RUN%" == "true" (
        echo %slimec% %FILE% -o %OUT_DIR%\project.asm
        echo %nasm% -fwin64 %OUT_DIR%\project.asm -o %OUT_DIR%\project.obj
        echo %link% /console /entry main kernel32.dll %OUT_DIR%\project.obj
    ) else (
        %slimec% %FILE% -o %OUT_DIR%\project.asm
        %nasm% -fwin64 %OUT_DIR%\project.asm -o %OUT_DIR%\project.obj
        %link% /console /entry main %link_files% %OUT_DIR%\project.obj
    )
    echo %FINISHED%
    goto EOF

:USAGE
    :: PRINT usage
    type usage.txt
    goto EOF

:FILE_NOT_EXIST
    :: File not exists. Calls error.
    call error FILE_NOT_EXIST %FILE%
    goto EOF

:EOF