project (cpp-wrapper)

set (PROJECT_INCLUDE_DIR ${PROJECT_SOURCE_DIR})
set (PROJECT_SOURCE_DIR ${CMAKE_CURRENT_SOURCE_DIR})
set (INTERCOM_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/../..)
set (TESTLIB_DIR ${INTERCOM_ROOT}/test/testlib)

# C++14 support
set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Define source files.
set(PROJECT_SRCS
${PROJECT_SOURCE_DIR}/main.cpp
${PROJECT_SOURCE_DIR}/exceptions.cpp
${PROJECT_SOURCE_DIR}/interface_wrappers.cpp
${PROJECT_SOURCE_DIR}/strings.cpp
)

include_directories("${PROJECT_BINARY_DIR}")
include_directories("${PROJECT_INCLUDE_DIR}")

# Ensure "intercom.hpp" is visible.
include_directories("${PROJECT_INCLUDE_DIR}/../../intercom-cpp")

add_executable(${PROJECT_NAME} ${PROJECT_SRCS} ${PROJECT_COMPILER_SPECIFIC_SRC})

# Linking to "runpath" library ensures that RUNPATH is set for ELF executables.
# All the test libraries built with Rust are copied into this location as well.
# Forcing the RUNPATH with the help of a dummy library ensures we can
# dynamically load the libraries with their name alone.
target_link_libraries(${PROJECT_NAME} PRIVATE runpath)

# Link to static dependencies
target_link_libraries(${PROJECT_NAME} PRIVATE cpp-utility)

# Add dynamic dependencies
target_link_libraries(${PROJECT_NAME} PRIVATE test_lib)
