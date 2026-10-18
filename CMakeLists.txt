cmake_minimum_required(VERSION 3.28)
project(Deck VERSION 0.1.0 LANGUAGES CXX)
include(CTest)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
find_program(DECK_CARGO_EXECUTABLE cargo REQUIRED)
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/scripts/locate_shape_source.py"
    --root "${CMAKE_SOURCE_DIR}" --cargo "${DECK_CARGO_EXECUTABLE}"
    RESULT_VARIABLE metadata_status OUTPUT_VARIABLE DECK_SHAPE_SOURCE OUTPUT_STRIP_TRAILING_WHITESPACE
    COMMAND_ERROR_IS_FATAL ANY)
if(NOT IS_DIRECTORY "${DECK_SHAPE_SOURCE}/apps/desktop")
    message(FATAL_ERROR "The locked Shape source could not be located")
endif()
message(STATUS "Deck consumes locked Shape source: ${DECK_SHAPE_SOURCE}")
add_subdirectory(apps/desktop)
