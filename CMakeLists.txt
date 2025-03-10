cmake_minimum_required(VERSION 3.10)
project(browser)

set(CMAKE_CXX_STANDARD 17)

# Explicitly add the include directory
include_directories(${CMAKE_SOURCE_DIR}/include)

# Find OpenSSL
set(OPENSSL_ROOT_DIR "C:/msys64/mingw64" CACHE PATH "Path to OpenSSL")
find_package(OpenSSL REQUIRED)

file(GLOB_RECURSE SOURCES "src/*.cpp" "include/*.cpp")
add_executable(browser ${SOURCES})

# Link OpenSSL
target_link_libraries(browser PRIVATE OpenSSL::SSL OpenSSL::Crypto ws2_32)
