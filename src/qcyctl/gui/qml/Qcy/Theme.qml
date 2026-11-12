pragma Singleton
import QtQuick

// Tokens do app. Componentes usam só estes valores.
QtObject {
    // Cores em #AARRGGBB (formato do QML). Painel noturno: fundo azul-escuro quase preto, luz de fundo azul-gelo (o LED da caixa).
    readonly property color bg: "#0B0D10"
    readonly property color surface: "#14171B"
    readonly property color surfaceHi: "#1C2026"
    readonly property color surfacePress: "#232831"
    readonly property color hairline: "#12FFFFFF"
    readonly property color edgeLight: "#1CFFFFFF"   // luz na borda superior do material
    readonly property color text: "#ECEDEE"
    readonly property color text2: "#A1A7AD"         // 7.3:1 sobre surface
    readonly property color text3: "#6E747B"         // só para desativado e decoração
    readonly property color accent: "#8CCBFF"        // 10:1 sobre surface
    readonly property color accentSoft: "#268CCBFF"
    readonly property color accentInk: "#0B1620"     // texto sobre accent
    readonly property color ok: "#4ADE80"
    readonly property color warn: "#FBBF24"
    readonly property color bad: "#F87171"

    readonly property color budTop: "#2B3038"
    readonly property color budBottom: "#101216"
    readonly property color tip: "#3A4048"

    // Tipografia: Inter (OFL), embutida.
    readonly property string family: "Inter"
    readonly property string display: "Inter Display"
    readonly property int sizeTitle: 26
    readonly property int sizeReadout: 22
    readonly property int sizeHeading: 15
    readonly property int sizeBody: 13
    readonly property int sizeLabel: 11

    // Espaçamento (base 4).
    readonly property int s1: 4
    readonly property int s2: 8
    readonly property int s3: 12
    readonly property int s4: 16
    readonly property int s5: 20
    readonly property int s6: 24
    readonly property int s8: 32
    readonly property int radius: 14
    readonly property int radiusSm: 9
    readonly property int control: 36                // altura de controles (mínimo 28 no desktop)

    // Movimento. Com "reduzir animações", só ficam fades curtos.
    property bool reduce: false
    readonly property int instant: 80
    readonly property int fast: 180
    readonly property int normal: 350
    readonly property int slow: 600
    function d(ms) { return reduce ? Math.min(ms, 150) : ms }
    // Molas (SpringAnimation). Amortecimento alto: sem overshoot visível em controles.
    readonly property real snappySpring: 5.0
    readonly property real snappyDamping: 0.5
    readonly property real gentleSpring: 3.0
    readonly property real gentleDamping: 0.45

    function batteryColor(pct) {
        if (pct < 0) return text3
        if (pct < 15) return bad
        if (pct <= 30) return warn
        return ok
    }
}
