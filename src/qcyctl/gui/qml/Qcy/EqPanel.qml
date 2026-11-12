import QtQuick
import Qcy

// Equalizador: presets e 10 bandas. A curva acompanha os controles.
Column {
    id: root
    width: parent ? parent.width : 300
    spacing: Theme.s3
    readonly property int n: backend.eqBands.length
    readonly property int range: backend.eqMax - backend.eqMin
    property real plotH: 112                    // altura da área do gráfico; o painel pode esticar
    // Altura de tudo o que não é a área do gráfico.
    readonly property real chrome: chips.height + hint.height + 2 * spacing + 56

    Flow {
        id: chips
        width: parent.width
        spacing: Theme.s2
        Repeater {
            model: backend.eqPresets
            Chip {
                required property int index
                required property string modelData
                text: modelData
                checked: backend.eqPreset === index
                onClicked: backend.setEqPreset(index)
            }
        }
        Chip {
            text: "Personalizado"
            checked: backend.eqPreset === 255
            visible: checked
        }
    }

    Item {
        id: graph
        width: parent.width
        height: root.plotH + 56
        readonly property real plotTop: 18
        readonly property real plotH: root.plotH
        readonly property real gutter: 40           // coluna das legendas de dB
        readonly property real colW: (width - gutter) / root.n
        function yFor(db) { return plotTop + (backend.eqMax - db) / root.range * plotH }

        // Linhas de referência: +8, 0, −8 dB.
        Repeater {
            model: [backend.eqMax, 0, backend.eqMin]
            Item {
                required property var modelData
                width: graph.width
                y: graph.yFor(modelData)
                Rectangle { x: graph.gutter; width: parent.width - graph.gutter; height: 1; color: modelData === 0 ? Theme.edgeLight : Theme.hairline }
                Text {
                    x: 0; y: -7
                    text: (modelData > 0 ? "+" : "") + modelData + " dB"
                    font.family: Theme.family; font.pixelSize: 10
                    font.features: { "tnum": 1 }
                    color: Theme.text2
                }
            }
        }

        Canvas {
            id: curve
            anchors.fill: parent
            renderStrategy: Canvas.Cooperative
            onPaint: {
                var ctx = getContext("2d")
                ctx.reset()
                var pts = []
                for (var i = 0; i < bands.count; i++) {
                    var it = bands.itemAt(i)
                    if (!it) return
                    pts.push({x: graph.gutter + (i + 0.5) * graph.colW, y: graph.yFor(it.shown)})
                }
                if (pts.length < 2) return
                function path() {
                    ctx.moveTo(pts[0].x, pts[0].y)
                    for (var k = 0; k < pts.length - 1; k++) {
                        var p0 = pts[Math.max(k - 1, 0)], p1 = pts[k], p2 = pts[k + 1], p3 = pts[Math.min(k + 2, pts.length - 1)]
                        ctx.bezierCurveTo(p1.x + (p2.x - p0.x) / 6, p1.y + (p2.y - p0.y) / 6,
                                          p2.x - (p3.x - p1.x) / 6, p2.y - (p3.y - p1.y) / 6, p2.x, p2.y)
                    }
                }
                var zero = graph.yFor(0)
                ctx.beginPath(); path()
                ctx.lineTo(pts[pts.length - 1].x, zero); ctx.lineTo(pts[0].x, zero); ctx.closePath()
                var g = ctx.createLinearGradient(0, graph.plotTop, 0, graph.plotTop + graph.plotH)
                g.addColorStop(0, "rgba(140,203,255,0.22)")
                g.addColorStop(0.5, "rgba(140,203,255,0.04)")
                g.addColorStop(1, "rgba(140,203,255,0.22)")
                ctx.fillStyle = g; ctx.fill()
                ctx.beginPath(); path()
                ctx.lineWidth = 2; ctx.strokeStyle = Theme.accent; ctx.stroke()
            }
        }

        Repeater {
            id: bands
            model: root.n
            Item {
                id: band
                required property int index
                x: graph.gutter + index * graph.colW
                width: graph.colW
                height: graph.height
                readonly property int gain: backend.eqGains[index] !== undefined ? backend.eqGains[index] : 0
                property real shown: gain
                Behavior on shown {
                    enabled: !Theme.reduce && !ma.pressed
                    NumberAnimation { duration: Theme.normal; easing.type: Easing.OutCubic }
                }
                onShownChanged: curve.requestPaint()
                activeFocusOnTab: true
                Keys.onUpPressed: backend.setEqGain(index, gain + 1)
                Keys.onDownPressed: backend.setEqGain(index, gain - 1)

                Rectangle {
                    id: thumb
                    width: 14; height: 14; radius: 7
                    x: (parent.width - width) / 2
                    y: graph.yFor(band.shown) - height / 2
                    color: ma.pressed || band.activeFocus ? Theme.accent : Theme.text
                    border.width: 3
                    border.color: Theme.bg
                    scale: ma.pressed ? 1.25 : (ma.containsMouse ? 1.1 : 1)
                    Behavior on scale { NumberAnimation { duration: Theme.instant } }
                }
                // Valor em dB enquanto arrasta ou com foco.
                Text {
                    anchors.horizontalCenter: thumb.horizontalCenter
                    y: thumb.y - 18
                    visible: ma.pressed || band.activeFocus
                    text: (band.gain > 0 ? "+" : "") + band.gain
                    font.family: Theme.family; font.pixelSize: 11; font.weight: Font.DemiBold
                    font.features: { "tnum": 1 }
                    color: Theme.accent
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: graph.plotTop + graph.plotH + 14
                    text: backend.eqBands[band.index]
                    font.family: Theme.family; font.pixelSize: 10; font.weight: Font.Medium
                    color: band.activeFocus ? Theme.accent : Theme.text2
                }
                MouseArea {
                    id: ma
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.SizeVerCursor
                    preventStealing: true
                    onPressed: (m) => { band.forceActiveFocus(Qt.MouseFocusReason); set(m.y) }
                    onPositionChanged: (m) => { if (pressed) set(m.y) }
                    function set(y) {
                        var db = Math.round(backend.eqMax - (y - graph.plotTop) / graph.plotH * root.range)
                        db = Math.max(backend.eqMin, Math.min(backend.eqMax, db))
                        if (db !== band.gain) backend.setEqGain(band.index, db)
                    }
                }
            }
        }
        Component.onCompleted: curve.requestPaint()
        onWidthChanged: curve.requestPaint()
        onHeightChanged: curve.requestPaint()
    }

    Text {
        id: hint
        width: parent.width
        wrapMode: Text.WordWrap
        text: "Arraste uma banda ou use as setas para cima e para baixo. A mudança vai para o fone na hora."
        font.family: Theme.family; font.pixelSize: Theme.sizeLabel + 1
        color: Theme.text2
    }
}
