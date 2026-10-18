# backplane-cef: built against a prebuilt CEF binary distribution (never
# Chromium from source). scripts/build-cef.sh fetches it and runs this.
cmake_minimum_required(VERSION 3.21)
project(backplane_cef C CXX)

if(NOT DEFINED CEF_ROOT)
  message(FATAL_ERROR "set -DCEF_ROOT=<unpacked cef_binary_*_minimal>")
endif()
set(CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} "${CEF_ROOT}/cmake")
set(USE_SANDBOX OFF)
find_package(CEF REQUIRED)

add_subdirectory(${CEF_LIBCEF_DLL_WRAPPER_PATH} libcef_dll_wrapper)

set(TARGET backplane-cef)
SET_CEF_TARGET_OUT_DIR()
add_executable(${TARGET} host.cc)
SET_EXECUTABLE_TARGET_PROPERTIES(${TARGET})
# see main() in host.cc: frames live across a zygote fork keep no canary
target_compile_options(${TARGET} PRIVATE -fno-stack-protector)
add_dependencies(${TARGET} libcef_dll_wrapper)
target_link_libraries(${TARGET} ${CEF_LIB_RELEASE} libcef_dll_wrapper ${CEF_STANDARD_LIBS} X11 dl)
# libcef.so sits beside the binary
set_target_properties(${TARGET} PROPERTIES INSTALL_RPATH "$ORIGIN" BUILD_WITH_INSTALL_RPATH TRUE)

COPY_FILES("${TARGET}" "${CEF_BINARY_FILES}" "${CEF_BINARY_DIR}" "${CEF_TARGET_OUT_DIR}")
COPY_FILES("${TARGET}" "${CEF_RESOURCE_FILES}" "${CEF_RESOURCE_DIR}" "${CEF_TARGET_OUT_DIR}")
