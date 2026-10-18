pragma Singleton

//! Deck keeps the related desktop controls and a quiet paper-first workspace.
//! Connected regions use separators; card artwork supplies the dominant color.

import QtQuick

QtObject {
    enum AppearanceMode {
        System,
        Light,
        Dark
    }

    property int mode: Theme.AppearanceMode.System
    property bool systemDark: false
    readonly property bool effectiveDark: mode === Theme.AppearanceMode.Dark
                                          || (mode === Theme.AppearanceMode.System
                                              && systemDark)

    // Stable desktop density shared with Shadow and Echo.
    readonly property int compactControlHeight: 30
    readonly property int controlHeight: 34
    readonly property int iconButtonSize: 32
    readonly property int iconSize: 20
    readonly property int compactIconSize: 18
    readonly property int toolbarHeight: 44
    readonly property int compactControlRadius: 6
    readonly property int controlRadius: 8
    readonly property int panelRadius: 12
    readonly property int cardRadius: 14
    readonly property int spaceXs: 4
    readonly property int spaceSm: 8
    readonly property int spaceMd: 12
    readonly property int spaceLg: 16
    readonly property int spaceXl: 24
    readonly property int panelPadding: 16
    readonly property int sectionSpacing: 14

    readonly property int fontMicro: 9
    readonly property int fontMeta: 10
    readonly property int fontBody: 12
    readonly property int fontSection: 11
    readonly property int fontHeading: 15
    readonly property int fontTitle: 18

    // Compatibility aliases keep existing semantic owners readable while the
    // workbench migrates to the shared desktop token vocabulary.
    readonly property int radiusSmall: compactControlRadius
    readonly property int radiusMedium: controlRadius
    readonly property int radiusLarge: panelRadius

    // Neutral surfaces let card artwork carry color in either appearance.
    readonly property color window: effectiveDark ? "#111111" : "#f2f2f2"
    readonly property color chrome: effectiveDark ? "#171717" : "#fafafa"
    readonly property color panel: effectiveDark ? "#1c1c1c" : "#f7f7f7"
    readonly property color panelRaised: effectiveDark ? "#242424" : "#ffffff"
    readonly property color panelInset: effectiveDark ? "#141414" : "#eeeeee"
    readonly property color canvas: effectiveDark ? "#121212" : "#e9e9e9"
    readonly property color canvasGrid: effectiveDark ? "#2b2b2b" : "#dadada"
    readonly property color surfaceSubtle: effectiveDark ? "#252525" : "#ededed"
    readonly property color surfaceSelected: effectiveDark ? "#333333" : "#dfdfdf"
    readonly property color control: effectiveDark ? "#222222" : "#ffffff"
    readonly property color controlQuiet: effectiveDark ? "#252525" : "#f1f1f1"
    readonly property color controlPressed: effectiveDark ? "#393939" : "#e2e2e2"

    readonly property color background: window
    readonly property color surface: panel
    readonly property color raised: panelRaised
    readonly property color raisedHover: effectiveDark ? "#303030" : "#eeeeee"
    readonly property color selected: surfaceSelected

    // Structure.
    readonly property color border: effectiveDark ? "#333333" : "#dddddd"
    readonly property color borderStrong: effectiveDark ? "#505050" : "#bcbcbc"
    readonly property color separatorStrong: effectiveDark ? "#494949" : "#b8b8b8"

    // Text and icons.
    readonly property color textPrimary: effectiveDark ? "#f4f4f4" : "#202020"
    readonly property color textSecondary: effectiveDark ? "#cccccc" : "#505050"
    readonly property color textMuted: effectiveDark ? "#a0a0a0" : "#6a6a6a"
    readonly property color textDisabled: effectiveDark ? "#777777" : "#a0a0a0"
    readonly property color textPlaceholder: effectiveDark ? "#8a8a8a" : "#808080"

    readonly property color text: textPrimary
    readonly property color textSoft: textSecondary
    readonly property color muted: textMuted
    readonly property color disabled: textDisabled

    // Monochrome emphasis and selection; error color is reserved for failures.
    readonly property color accent: effectiveDark ? "#ededed" : "#242424"
    readonly property color accentHover: effectiveDark ? "#ffffff" : "#3d3d3d"
    readonly property color accentPressed: effectiveDark ? "#c9c9c9" : "#101010"
    readonly property color accentSoft: effectiveDark ? "#333333" : "#e5e5e5"
    readonly property color accentSurfaceQuiet: effectiveDark ? "#282828" : "#f0f0f0"
    readonly property color accentBorder: effectiveDark ? "#898989" : "#888888"
    readonly property color accentText: effectiveDark ? "#181818" : "#ffffff"
    readonly property color accentSelectionText: effectiveDark ? "#ffffff" : "#202020"

    readonly property color success: effectiveDark ? "#c7c7c7" : "#4b4b4b"
    readonly property color successSoft: effectiveDark ? "#282828" : "#eeeeee"
    readonly property color warning: effectiveDark ? "#cccccc" : "#555555"
    readonly property color warningSoft: effectiveDark ? "#292929" : "#eaeaea"
    readonly property color danger: effectiveDark ? "#e4aaaa" : "#a23b3b"
    readonly property color dangerSoft: effectiveDark ? "#352727" : "#f5e8e8"

    // Controls and elevation. Shadows are reserved for floating palettes and
    // dialogs; connected workbench regions rely on separators.
    readonly property color buttonSurface: effectiveDark ? "#292929" : "#ffffff"
    readonly property color buttonHoverSurface: effectiveDark ? "#353535" : "#eeeeee"
    readonly property color buttonPressedSurface: effectiveDark ? "#434343" : "#dfdfdf"
    readonly property color buttonGhostHover: effectiveDark ? "#2d2d2d" : "#e9e9e9"
    readonly property color buttonGhostPressed: effectiveDark ? "#3b3b3b" : "#dcdcdc"
    readonly property color buttonBorder: effectiveDark ? "#535353" : "#cbcbcb"
    readonly property color focusRing: effectiveDark ? "#c5c5c5" : "#515151"
    readonly property color shadow: effectiveDark ? "#66000000" : "#22000000"
    readonly property color shadowSoft: effectiveDark ? "#44000000" : "#16000000"
    readonly property color transparent: "#00000000"
}
