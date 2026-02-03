@echo off

set ERROR_SUFFIX=Error:

if "%1"=="file_not_exist" goto not_exist

:not_exist
echo %ERROR_SUFFIX% File "%2" not exist
goto EOF

:EOF