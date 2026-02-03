@echo off

set WARNING_SUFFIX=Warning:

if "%1"=="not_def" goto not_def

:not_def
echo %WARNING_SUFFIX%: environment "%2" not defined
goto EOF

:EOF