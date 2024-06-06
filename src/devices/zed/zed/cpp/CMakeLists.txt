CMAKE_MINIMUM_REQUIRED(VERSION 3.5)
PROJECT(ZED_Interface_Project)

set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

option(LINK_SHARED_ZED "Link with the ZED SDK shared executable" ON)

if (NOT LINK_SHARED_ZED AND MSVC)
    message(FATAL_ERROR "LINK_SHARED_ZED OFF : ZED SDK static libraries not available on Windows")
endif()

find_package(ZED 3 REQUIRED)
find_package(CUDA ${ZED_CUDA_VERSION} REQUIRED)

include_directories(${CUDA_INCLUDE_DIRS})
include_directories(${ZED_INCLUDE_DIRS})

link_directories(${ZED_LIBRARY_DIR})
link_directories(${CUDA_LIBRARY_DIRS})

# Create a shared library for the zed_interface
ADD_LIBRARY(zed_interface_lib SHARED zed_interface.cpp)

# Create an executable for zed_interface_main
ADD_EXECUTABLE(zed_interface_main zed_interface_main.cpp)

if (LINK_SHARED_ZED)
    SET(ZED_LIBS ${ZED_LIBRARIES} ${CUDA_CUDA_LIBRARY} ${CUDA_CUDART_LIBRARY})
else()
    SET(ZED_LIBS ${ZED_STATIC_LIBRARIES} ${CUDA_CUDA_LIBRARY} ${CUDA_LIBRARY})
endif()

# Link the ZED and CUDA libraries with both the shared library and the executable
TARGET_LINK_LIBRARIES(zed_interface_lib ${ZED_LIBS})
TARGET_LINK_LIBRARIES(zed_interface_main zed_interface_lib ${ZED_LIBS})
