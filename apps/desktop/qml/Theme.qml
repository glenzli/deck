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
    property bool systemDark: true
    readonly property bool effectiveDark: mode === Theme.AppearanceMode.Dark
                                          || (mode === Theme.AppearanceMode.System
                                              && systemDark)

    // Stable desktop density shared with Shadow and Echo.
    readonly property int compactControlHeight: 30
    readonly property int controlHeight: 34
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

    // Base surfaces. Light mode stays cool grey-white instead of warm or
    // yellow; depth comes from restrained contrast rather than boxed shadows.
    readonly property color window: effectiveDark ? "#101214" : "#f1f3f5"
    readonly property color chrome: effectiveDark ? "#141619" : "#fcfcfd"
    readonly property color panel: effectiveDark ? "#181b1f" : "#f8f9fa"
    readonly property color panelRaised: effectiveDark ? "#1e2227" : "#ffffff"
    readonly property color panelInset: effectiveDark ? "#13171b" : "#eef1f4"
    readonly property color canvas: effectiveDark ? "#15191d" : "#edf0f2"
    readonly property color canvasGrid: effectiveDark ? "#262c32" : "#dfe4e9"
    readonly property color surfaceSubtle: effectiveDark ? "#20252b" : "#f1f3f5"
    readonly property color surfaceSelected: effectiveDark ? "#252d35" : "#e7edf3"
    readonly property color control: effectiveDark ? "#1d2228" : "#ffffff"
    readonly property color controlQuiet: effectiveDark ? "#20252b" : "#f3f5f7"
    readonly property color controlPressed: effectiveDark ? "#2b323a" : "#e8edf1"

    readonly property color background: window
    readonly property color surface: panel
    readonly property color raised: panelRaised
    readonly property color raisedHover: effectiveDark ? "#252b32" : "#f1f4f6"
    readonly property color selected: surfaceSelected

    // Structure.
    readonly property color border: effectiveDark ? "#30363d" : "#e1e5e9"
    readonly property color borderStrong: effectiveDark ? "#48515b" : "#c7d0d8"
    readonly property color separatorStrong: effectiveDark ? "#404850" : "#bcc5cd"

    // Text and icons.
    readonly property color textPrimary: effectiveDark ? "#f1f4f6" : "#20252a"
    readonly property color textSecondary: effectiveDark ? "#cbd3d9" : "#4f5964"
    readonly property color textMuted: effectiveDark ? "#98a4af" : "#68737e"
    readonly property color textDisabled: effectiveDark ? "#6f7c88" : "#a3abb4"
    readonly property color textPlaceholder: effectiveDark ? "#75818c" : "#7e8993"

    readonly property color text: textPrimary
    readonly property color textSoft: textSecondary
    readonly property color muted: textMuted
    readonly property color disabled: textDisabled

    // Shape uses blue for authored or pending creative state. Accepted output
    // remains a quieter green signal and does not compete with primary actions.
    readonly property color accent: effectiveDark ? "#70a3d6" : "#3574b9"
    readonly property color accentHover: effectiveDark ? "#82b2df" : "#2865a6"
    readonly property color accentPressed: effectiveDark ? "#5d91c6" : "#285f9c"
    readonly property color accentSoft: effectiveDark ? "#1d3246" : "#eaf2fa"
    readonly property color accentSurfaceQuiet: effectiveDark ? "#192736" : "#f3f7fb"
    readonly property color accentBorder: effectiveDark ? "#5e95c4" : "#91b4d8"
    readonly property color accentText: "#ffffff"
    readonly property color accentSelectionText: effectiveDark ? "#b9d6ee" : "#285f9c"

    readonly property color success: effectiveDark ? "#7db68b" : "#3d7850"
    readonly property color successSoft: effectiveDark ? "#1c3024" : "#e9f3ec"
    readonly property color warning: effectiveDark ? "#c5a66a" : "#8a651d"
    readonly property color warningSoft: effectiveDark ? "#352f22" : "#f8efd9"
    readonly property color danger: effectiveDark ? "#d77f79" : "#a5413d"
    readonly property color dangerSoft: effectiveDark ? "#3c2625" : "#f8e8e6"

    // Controls and elevation. Shadows are reserved for floating palettes and
    // dialogs; connected workbench regions rely on separators.
    readonly property color buttonSurface: effectiveDark ? "#252d36" : "#ffffff"
    readonly property color buttonHoverSurface: effectiveDark ? "#303b47" : "#f3f6f8"
    readonly property color buttonPressedSurface: effectiveDark ? "#3b4856" : "#e9edf2"
    readonly property color buttonGhostHover: effectiveDark ? "#252f39" : "#eef2f5"
    readonly property color buttonGhostPressed: effectiveDark ? "#303c48" : "#e5eaef"
    readonly property color buttonBorder: effectiveDark ? "#53616e" : "#d1d8e0"
    readonly property color focusRing: effectiveDark ? "#76a6d4" : "#5a91cb"
    readonly property color shadow: effectiveDark ? "#66000000" : "#22131b24"
    readonly property color shadowSoft: effectiveDark ? "#44000000" : "#1618202a"
    readonly property color transparent: "#00000000"
}
