cmake_minimum_required(VERSION 3.25)

project(tilescript LANGUAGES CXX)

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(TILESCRIPT_ROOT "${CMAKE_CURRENT_SOURCE_DIR}" CACHE PATH "tilescript repository root")
set(TILESCRIPT_FFI_TARGET_DIR "${TILESCRIPT_ROOT}/target/release" CACHE PATH "Rust build output directory")
set(TILESCRIPT_FFI_STATICLIB "${TILESCRIPT_FFI_TARGET_DIR}/libtilescript_ffi.a")
set(TILESCRIPT_HYPR_OUTPUT_NAME "tilescript-hypr" CACHE STRING "Output name for the Hyprland plugin shared object")
set(HYPRLAND_SOURCE_DIR "/usr/include/hyprland" CACHE PATH "Path to installed Hyprland headers or a Hyprland checkout root")
set(HYPRLAND_PKG_CONFIG_DIR "" CACHE PATH "Optional pkg-config directory containing hyprland.pc for the Hyprland build under test")

find_package(PkgConfig REQUIRED)
if(HYPRLAND_PKG_CONFIG_DIR)
  set(ENV{PKG_CONFIG_PATH} "${HYPRLAND_PKG_CONFIG_DIR}:$ENV{PKG_CONFIG_PATH}")
endif()
pkg_check_modules(HYPRLAND REQUIRED hyprland)
pkg_check_modules(JSONCPP REQUIRED jsoncpp)
pkg_check_modules(PIXMAN REQUIRED pixman-1)
pkg_check_modules(LIBDRM REQUIRED libdrm)

find_program(CARGO_EXECUTABLE cargo REQUIRED)

add_custom_target(
  tilescript_ffi ALL
  COMMAND "${CARGO_EXECUTABLE}" build --release -p tilescript-ffi
  WORKING_DIRECTORY "${TILESCRIPT_ROOT}"
  COMMENT "Building tilescript-ffi release staticlib"
  VERBATIM
)

add_library(tilescript-hypr SHARED plugin/hyprland/src/plugin.cpp plugin/hyprland/src/plugin_algorithm.cpp plugin/hyprland/src/plugin_dispatchers.cpp plugin/hyprland/src/plugin_hooks.cpp plugin/hyprland/src/plugin_query.cpp plugin/hyprland/src/plugin_runtime.cpp plugin/hyprland/src/plugin_sync.cpp)
add_dependencies(tilescript-hypr tilescript_ffi)

target_include_directories(
  tilescript-hypr
  PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
    "${CMAKE_CURRENT_SOURCE_DIR}/plugin/hyprland/include"
    "${HYPRLAND_SOURCE_DIR}"
    "${HYPRLAND_SOURCE_DIR}/protocols"
    "${HYPRLAND_SOURCE_DIR}/src"
    ${HYPRLAND_INCLUDE_DIRS}
    ${JSONCPP_INCLUDE_DIRS}
    ${PIXMAN_INCLUDE_DIRS}
    ${LIBDRM_INCLUDE_DIRS}
)

target_link_libraries(tilescript-hypr PRIVATE "${TILESCRIPT_FFI_STATICLIB}" ${HYPRLAND_LIBRARIES} ${JSONCPP_LIBRARIES} ${PIXMAN_LIBRARIES} ${LIBDRM_LIBRARIES})
target_compile_options(tilescript-hypr PRIVATE ${HYPRLAND_CFLAGS_OTHER} ${JSONCPP_CFLAGS_OTHER} ${PIXMAN_CFLAGS_OTHER} ${LIBDRM_CFLAGS_OTHER})

set_target_properties(
  tilescript-hypr
  PROPERTIES
    PREFIX ""
    OUTPUT_NAME "${TILESCRIPT_HYPR_OUTPUT_NAME}"
)
