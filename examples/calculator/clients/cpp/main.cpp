// main.cpp : Defines the entry point for the console application.
//

#define _WIN32_DCOM

#include <stdio.h>
#include <tchar.h>

#include <comdef.h>

#include "calculator_h.h"
#include <iostream>

#pragma comment(linker, "\"/manifestdependency:name='Calculator.X' type='win32' version='1.0.0.0'\"" )

#define IS_OK( hr ) if( IS_ERROR( hr ) ) { return -1; }

int main()
{
	// Initialize COM.
	CoInitializeEx( nullptr, COINIT_APARTMENTTHREADED );
	
	// Get the ICalculator interface.
	ICalculator* pCalculator = nullptr;
	HRESULT hr = CoCreateInstance(
			CLSID_Calculator,
			nullptr,
			CLSCTX_INPROC_SERVER,
			IID_ICalculator,
			reinterpret_cast<void**>(&pCalculator) );

	int value = 0;
	IS_OK(pCalculator->Add(10, &value));       // 0 + 10 = 10
	IS_OK(pCalculator->Add(5, &value));        // 10 + 5 = 15
	IS_OK(pCalculator->Multiply(2, &value));   // 15 * 2 = 30
	IS_OK(pCalculator->Substract(8, &value));  // 30 - 8 = 22

	std::cout << "(10 + 5) * 2 - 8 = " << value << std::endl;

	// Shut down COM.
	CoUninitialize();
    return 0;
}

