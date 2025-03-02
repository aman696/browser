cmake_minimum_required(VERSION 3.10)
project(browser)

set(CMAKE_CXX_STANDARD 17)

include_directories(include)
file(GLOB_RECURSE SOURCES "src/*.cpp")

add_executable(browser ${SOURCES})
