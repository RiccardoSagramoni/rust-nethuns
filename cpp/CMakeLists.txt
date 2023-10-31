# Copyright 2021 Larthia, University of Pisa. All rights reserved.
# Use of this source code is governed by a BSD-style
# license that can be found in the LICENSE file.

cmake_minimum_required(VERSION 3.0)

project (nethuns-test)


#
# Nethuns options...
#
set(NETHUNS_LIBS)
    
find_library(LIBPCAP_LIBRARY pcap)
find_library(LIBBPF_LIBRARY bpf)
find_library(LIBELF_LIBRARY elf)
find_library(LIBNETMAP_LIBRARY netmap)
find_package(ZLIB)

list(APPEND NETHUNS_LIBS nethuns)
list(APPEND NETHUNS_LIBS ${LIBPCAP_LIBRARY})

message ("Nethuns: netmap socket enabled!")
add_definitions(-DNETHUNS_SOCKET=1)
list(APPEND NETHUNS_LIBS ${LIBNETMAP_LIBRARY})


#
# Compiler options...
#
set (CMAKE_C_FLAGS   "${CMAKE_C_FLAGS}   -g -O2 -march=native -fomit-frame-pointer -Wall -Wextra -Wshadow")
set (CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -g -O2 -std=c++17 -march=native -fomit-frame-pointer -Wall -Wextra -Wshadow")

find_library(LIBPCAP_LIBRARY NAMES libpcap.a PATHS "/usr/local/opt/licap/lib" NO_DEFAULT_PATH NO_SYSTEM_ENVIRONMENT_PATH)
find_path(LIBPCAP_INCLUDE_DIR NAMES pcap/pcap.h PATHS "/usr/local/opt/libpcap/include/" NO_DEFAULT_PATH_ENVIRONMENT_PATH)

include_directories(/usr/local/lib .)
link_directories(/usr/local/lib)

add_executable(nethuns-perf_recv      src/perf_recv.cpp)
add_executable(nethuns-perf_send      src/perf_send.cpp)

target_link_libraries(nethuns-perf_recv   ${NETHUNS_LIBS} -pthread)
target_link_libraries(nethuns-perf_send   ${NETHUNS_LIBS} -pthread)
