# backplane-step2glb: the STEP mesher (step2glb.cpp), against OpenCascade found by CMake: OpenCASCADE_DIR is the folder of its
# OpenCASCADEConfig.cmake (scripts/build-occt.sh's static build for a release, the system's for a dev build). scripts/build-step2glb.sh
# runs this.
cmake_minimum_required(VERSION 3.16)
project(backplane_step2glb CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
find_package(OpenCASCADE REQUIRED CONFIG)
find_package(Threads REQUIRED)
add_executable(backplane-step2glb step2glb.cpp)
target_include_directories(backplane-step2glb PRIVATE ${OpenCASCADE_INCLUDE_DIR})
# the toolkits it calls; their own dependencies come with their imported targets
target_link_libraries(backplane-step2glb PRIVATE TKXCAF TKDESTEP TKDE TKXSBase TKVCAF TKCAF TKLCAF TKCDF TKV3d TKService TKMesh
  TKShHealing TKTopAlgo TKGeomAlgo TKBRep TKGeomBase TKG3d TKG2d TKMath TKernel Threads::Threads)
if(OpenCASCADE_BUILD_SHARED_LIBS STREQUAL "OFF" OR OCCT_STATIC)
  # a static OpenCascade: its headers must not ask for DLL imports (Windows), and the program carries all of it
  target_compile_definitions(backplane-step2glb PRIVATE OCCT_STATIC_BUILD)
  if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    target_link_libraries(backplane-step2glb PRIVATE ${CMAKE_DL_LIBS})
  endif()
endif()
if(MSVC)
  target_compile_options(backplane-step2glb PRIVATE /utf-8 /EHsc)
endif()
install(TARGETS backplane-step2glb RUNTIME DESTINATION .)
