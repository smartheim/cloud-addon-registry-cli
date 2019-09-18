cmake_minimum_required(VERSION 3.9.4)

project(addon CXX)

include(CheckIPOSupported)
check_ipo_supported(RESULT supported OUTPUT error)

SET(CMAKE_FIND_LIBRARY_SUFFIXES ".a")
SET(BUILD_SHARED_LIBS OFF)
SET(CMAKE_EXE_LINKER_FLAGS "-static -static-libgcc")
#target_link_libraries(${PROJECT} --static)
add_executable(${PROJECT} app main.cpp)

if( supported )
    message(STATUS "IPO / LTO enabled")
    set_property(TARGET ${PROJECT} PROPERTY INTERPROCEDURAL_OPTIMIZATION TRUE)
else()
    message(STATUS "IPO / LTO not supported: <${error}>")
endif()
