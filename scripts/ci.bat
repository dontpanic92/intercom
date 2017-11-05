setlocal enableextensions
FOR /F "tokens=* delims= " %%i in ( 'scripts\vswhere -format value -property installationPath' ) do ( set installdir=%%i )
pushd %installdir%
call .\VC\Auxiliary\Build\vcvars64.bat
popd

REM Build Intercom
cargo build
if %errorlevel% neq 0 exit /b %errorlevel%

REM Build testlib
pushd test\testlib

cargo build
if %errorlevel% neq 0 exit /b %errorlevel%

popd

REM Generate IDL and Manifest for the testlib.
pushd intercom_utils

cargo run -- idl ..\test\testlib > ..\test\testlib\testlib.idl
if %errorlevel% neq 0 exit /b %errorlevel%

cargo run -- manifest ..\test\testlib > ..\test\testlib\TestLib.Assembly.manifest
if %errorlevel% neq 0 exit /b %errorlevel%

popd

REM Build C++ test suite
pushd test\cpp

devenv cpp.sln /Build "Release|x64"
if %errorlevel% neq 0 exit /b %errorlevel%

popd

