find_package(Qt6 6.9 REQUIRED COMPONENTS Concurrent Multimedia Quick QuickControls2 Svg LinguistTools)
find_package(Threads REQUIRED)
qt_standard_project_setup(REQUIRES 6.9 I18N_TRANSLATED_LANGUAGES zh_CN)
qt_policy(SET QTP0004 NEW)
set(upstream "${DECK_SHAPE_SOURCE}/apps/desktop")
set(bridge_target "${CMAKE_BINARY_DIR}/shape-cargo-target")
set(bridge_include "${CMAKE_BINARY_DIR}/cxxbridge")
set(cargo_target "${CMAKE_BINARY_DIR}/cargo-target")
if(CMAKE_BUILD_TYPE STREQUAL "Release")
    set(profile release)
    set(release_argument --release)
else()
    set(profile debug)
endif()
if(WIN32)
    set(bridge_archive "${bridge_target}/${profile}/shape_desktop_bridge.lib")
    set(cli_binary "${cargo_target}/${profile}/deck-cli.exe")
else()
    set(bridge_archive "${bridge_target}/${profile}/libshape_desktop_bridge.a")
    set(cli_binary "${cargo_target}/${profile}/deck-cli")
endif()
set(cargo_environment "${CMAKE_COMMAND}" -E env CARGO_BUILD_JOBS=2)
if(APPLE)
    list(APPEND cargo_environment MACOSX_DEPLOYMENT_TARGET=26.0)
endif()
file(GLOB_RECURSE rust_inputs CONFIGURE_DEPENDS "${DECK_SHAPE_SOURCE}/crates/*/src/*.rs" "${DECK_SHAPE_SOURCE}/crates/*/Cargo.toml" "${DECK_SHAPE_SOURCE}/crates/*/build.rs")
add_custom_command(OUTPUT "${bridge_archive}"
    BYPRODUCTS "${bridge_include}/rust/cxx.h" "${bridge_include}/shape-desktop-bridge/src/lib.rs.h"
    COMMAND ${cargo_environment} "${Python3_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/scripts/build_shape_bridge.py"
        --source "${DECK_SHAPE_SOURCE}" --target "${bridge_target}" --include "${bridge_include}"
        --cargo "${DECK_CARGO_EXECUTABLE}" ${release_argument}
    DEPENDS ${rust_inputs} "${CMAKE_SOURCE_DIR}/scripts/build_shape_bridge.py" "${DECK_SHAPE_SOURCE}/Cargo.lock"
    VERBATIM)
add_custom_target(deck-rust-bridge-build DEPENDS "${bridge_archive}")
add_library(deck-rust-bridge STATIC IMPORTED GLOBAL)
set_target_properties(deck-rust-bridge PROPERTIES IMPORTED_LOCATION "${bridge_archive}")
add_dependencies(deck-rust-bridge deck-rust-bridge-build)
file(GLOB_RECURSE cli_inputs CONFIGURE_DEPENDS "${CMAKE_SOURCE_DIR}/apps/deck-cli/src/*.rs" "${CMAKE_SOURCE_DIR}/apps/deck-cli/Cargo.toml")
add_custom_command(OUTPUT "${cli_binary}"
    COMMAND ${cargo_environment} "${DECK_CARGO_EXECUTABLE}" build --locked --manifest-path "${CMAKE_SOURCE_DIR}/Cargo.toml"
        -p deck-cli --target-dir "${cargo_target}" ${release_argument}
    DEPENDS ${cli_inputs} "${CMAKE_SOURCE_DIR}/Cargo.toml" "${CMAKE_SOURCE_DIR}/Cargo.lock"
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}" VERBATIM)
add_custom_target(deck-cli-build DEPENDS "${cli_binary}")
qt_add_executable(deck-desktop
    src/main.cpp src/deck_cli_client.cpp src/deck_cli_client.hpp
    src/deck_document_controller.cpp src/deck_document_controller.hpp
    src/deck_exchange_controller.cpp src/deck_exchange_controller.hpp
    src/external_image_task_controller.cpp src/external_image_task_controller.hpp
    src/pdf_command.cpp src/pdf_command.hpp src/process_liveness.cpp src/process_liveness.hpp
    src/card_print_export.cpp src/card_print_export.hpp
    tests/deck_native_smoke.cpp tests/deck_native_smoke.hpp
    "${upstream}/src/desktop_backend.cpp" "${upstream}/src/desktop_backend.hpp"
    "${upstream}/src/card_kit_controller.cpp" "${upstream}/src/card_kit_controller.hpp"
    src/card_print_controller.cpp src/card_print_controller.hpp
    "${upstream}/src/material_edit_controller.cpp" "${upstream}/src/material_edit_controller.hpp"
    "${upstream}/src/audio_preview_controller.cpp" "${upstream}/src/audio_preview_controller.hpp"
    "${upstream}/src/image_preview_provider.cpp" "${upstream}/src/image_preview_provider.hpp"
    "${upstream}/src/ui_preferences.cpp" "${upstream}/src/ui_preferences.hpp")
set_source_files_properties(qml/Theme.qml PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
file(GLOB_RECURSE icons CONFIGURE_DEPENDS RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}" "icons/*.svg")
qt_add_qml_module(deck-desktop URI Shape.Desktop VERSION 1.0
    QML_FILES qml/DeckWindow.qml qml/Theme.qml
        qml/components/CardKitWorkspace.qml qml/components/CardLayoutOptions.qml
        qml/components/CardImageIntakeDialog.qml qml/components/CardPrintDialog.qml
        qml/components/CardListImport.js qml/components/MaterialEditWorkspace.qml qml/components/ImageCompareWorkspace.qml
        qml/components/ShapeButton.qml qml/components/ShapeTextField.qml
        qml/components/ShapeTextArea.qml qml/components/ShapeTextEditor.qml
        qml/components/ShapeComboBox.qml qml/components/ShapeDialog.qml
        qml/components/ShapeIcon.qml qml/components/ShapeIconButton.qml
    RESOURCES ${icons})
qt_add_translations(deck-desktop TS_FILES translations/shape_zh_CN.ts RESOURCE_PREFIX "/translations"
    LUPDATE_OPTIONS -no-obsolete -locations none LRELEASE_OPTIONS -nounfinished)
target_compile_features(deck-desktop PRIVATE cxx_std_20)
target_include_directories(deck-desktop PRIVATE src tests "${upstream}/src" "${bridge_include}")
target_link_libraries(deck-desktop PRIVATE Qt6::Quick Qt6::QuickControls2 Qt6::Concurrent Qt6::Multimedia Qt6::Svg deck-rust-bridge Threads::Threads ${CMAKE_DL_LIBS})
add_dependencies(deck-desktop deck-rust-bridge-build deck-cli-build)
if(MSVC)
    target_compile_options(deck-desktop PRIVATE /W4 /permissive-)
else()
    target_compile_options(deck-desktop PRIVATE -Wall -Wextra -Wpedantic)
endif()
set_target_properties(deck-desktop PROPERTIES MACOSX_BUNDLE TRUE WIN32_EXECUTABLE TRUE
    MACOSX_BUNDLE_BUNDLE_NAME "Deck" MACOSX_BUNDLE_GUI_IDENTIFIER "dev.glenzli.deck")
add_custom_target(deck-package-cli ALL COMMAND "${CMAKE_COMMAND}" -E copy_if_different
    "${cli_binary}" "$<TARGET_FILE_DIR:deck-desktop>/deck-cli${CMAKE_EXECUTABLE_SUFFIX}"
    DEPENDS deck-desktop deck-cli-build "${cli_binary}" VERBATIM)
install(TARGETS deck-desktop BUNDLE DESTINATION . RUNTIME DESTINATION bin)
install(PROGRAMS "${cli_binary}" DESTINATION bin)
if(BUILD_TESTING)
    add_test(NAME deck-native-smoke COMMAND $<TARGET_FILE:deck-desktop> --smoke-native)
    set_tests_properties(deck-native-smoke PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen" TIMEOUT 120
        FAIL_REGULAR_EXPRESSION "QQml;ReferenceError;TypeError;Binding loop;failed to load")
endif()
