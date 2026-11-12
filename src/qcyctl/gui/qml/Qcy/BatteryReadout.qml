import QtQuick
import Qcy

// Leitura de bateria acima de cada peça: número, nome e barra.
// A barra anima com mola; em carga, a barra pulsa devagar.
Item {
    id: root
    property string label: "Esquerdo"
    property int percent: -1          // -1 = sem leitura
    property bool charging: false
    property real shown: Math.max(percent, 0)
    implicitWidth: 96
    implicitHeight: col.implicitHeight

    readonly property color tone: Theme.batteryColor(percent)

    Behavior on shown {
        enabled: !Theme.reduce
        SpringAnimation { spring: Theme.gentleSpring; damping: Theme.gentleDamping; epsilon: 0.2 }
    }

    // Varredura de ignição: ao conectar, as barras sobem a 100 e voltam ao valor real.
    function sweep() {
        if (Theme.reduce) return
        sweepAnim.restart()
    }
    SequentialAnimation {
        id: sweepAnim
        PropertyAction { target: root; property: "shown"; value: 0 }
        PauseAnimation { duration: 120 }
        NumberAnimation { target: root; property: "shown"; to: 100; duration: 520; easing.type: Easing.OutCubic }
        PauseAnimation { duration: 80 }
        NumberAnimation { target: root; property: "shown"; to: Math.max(root.percent, 0); duration: 700; easing.type: Easing.InOutCubic }
        ScriptAction { script: root.shown = Qt.binding(function() { return Math.max(root.percent, 0) }) }
    }

    Column {
        id: col
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: 6

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 3
            Text {
                text: root.percent < 0 ? "—" : Math.round(root.sweepRunning ? root.shown : root.percent)
                font.family: Theme.display
                font.weight: Font.DemiBold
                font.pixelSize: Theme.sizeReadout
                font.features: { "tnum": 1 }
                color: root.percent < 0 ? Theme.text3 : Theme.text
            }
            Text {
                visible: root.percent >= 0
                text: "%"
                anchors.baseline: parent.children[0].baseline
                font.family: Theme.family
                font.weight: Font.Medium
                font.pixelSize: Theme.sizeBody
                color: Theme.text2
            }
            Icon {
                visible: root.charging
                name: "bolt"
                color: Theme.accent
                width: 13; height: 13
                anchors.verticalCenter: parent.verticalCenter
            }
        }

        Rectangle {
            id: track
            anchors.horizontalCenter: parent.horizontalCenter
            width: 56; height: 4; radius: 2
            color: Theme.surfaceHi
            Rectangle {
                height: parent.height; radius: 2
                width: parent.width * Math.min(root.shown, 100) / 100
                color: root.tone
                SequentialAnimation on opacity {
                    running: root.charging && Theme.loop
                    loops: Animation.Infinite
                    alwaysRunToEnd: true
                    NumberAnimation { to: 0.45; duration: 1200; easing.type: Easing.InOutSine }
                    NumberAnimation { to: 1; duration: 1200; easing.type: Easing.InOutSine }
                }
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.charging ? root.label + " · carregando" : root.label
            font.family: Theme.family
            font.pixelSize: Theme.sizeLabel
            font.weight: Font.Medium
            color: Theme.text2
        }
    }
    readonly property bool sweepRunning: sweepAnim.running
}
