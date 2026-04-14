cmake_minimum_required(VERSION 3.25)

project(hypreact LANGUAGES CXX)

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(HYPREACT_ROOT "${CMAKE_CURRENT_SOURCE_DIR}" CACHE PATH "hypreact repository root")
set(HYPREACT_FFI_TARGET_DIR "${HYPREACT_ROOT}/target/release" CACHE PATH "Rust build output directory")
set(HYPREACT_FFI_STATICLIB "${HYPREACT_FFI_TARGET_DIR}/libhypreact_hypr_ffi.a")
set(HYPRLAND_SOURCE_DIR "/usr/include/hyprland" CACHE PATH "Path to installed Hyprland headers or a Hyprland checkout root")

find_package(PkgConfig REQUIRED)
pkg_check_modules(HYPRLAND REQUIRED hyprland)
pkg_check_modules(JSONCPP REQUIRED jsoncpp)
pkg_check_modules(PIXMAN REQUIRED pixman-1)
pkg_check_modules(LIBDRM REQUIRED libdrm)

find_program(CARGO_EXECUTABLE cargo REQUIRED)

add_custom_target(
  hypreact_ffi ALL
  COMMAND "${CARGO_EXECUTABLE}" build --release -p hypreact-hypr-ffi
  WORKING_DIRECTORY "${HYPREACT_ROOT}"
  COMMENT "Building hypreact-hypr-ffi release staticlib"
  VERBATIM
)

add_library(hypreact SHARED plugin/hyprland/src/plugin.cpp plugin/hyprland/src/plugin_algorithm.cpp plugin/hyprland/src/plugin_dispatchers.cpp plugin/hyprland/src/plugin_hooks.cpp plugin/hyprland/src/plugin_query.cpp plugin/hyprland/src/plugin_runtime.cpp plugin/hyprland/src/plugin_sync.cpp)
add_dependencies(hypreact hypreact_ffi)

target_include_directories(
  hypreact
  PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
    "${CMAKE_CURRENT_SOURCE_DIR}/plugin/hyprland/include"
    "${HYPRLAND_SOURCE_DIR}"
    ${HYPRLAND_INCLUDE_DIRS}
    ${JSONCPP_INCLUDE_DIRS}
    ${PIXMAN_INCLUDE_DIRS}
    ${LIBDRM_INCLUDE_DIRS}
)

target_link_libraries(hypreact PRIVATE "${HYPREACT_FFI_STATICLIB}" ${HYPRLAND_LIBRARIES} ${JSONCPP_LIBRARIES} ${PIXMAN_LIBRARIES} ${LIBDRM_LIBRARIES})
target_compile_options(hypreact PRIVATE ${HYPRLAND_CFLAGS_OTHER} ${JSONCPP_CFLAGS_OTHER} ${PIXMAN_CFLAGS_OTHER} ${LIBDRM_CFLAGS_OTHER})

set_target_properties(
  hypreact
  PROPERTIES
    PREFIX ""
    OUTPUT_NAME "hypreact"
)
