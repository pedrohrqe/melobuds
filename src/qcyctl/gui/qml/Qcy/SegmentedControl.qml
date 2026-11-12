import QtQuick
import Qcy

// Controle segmentado: a pílula desliza com mola até a opção ativa.
// Setas esquerda/direita mudam a opção pelo teclado.
FocusScope {
    id: root
    property var options: []          // textos
    property int current: -1
    signal picked(int index)
    implicitHeight: Theme.control
    implicitWidth: 300
    activeFocusOnTab: true

    readonly property real segW: options.length ? (width - 6) / options.length : 0

    Keys.onLeftPressed: if (current > 0) picked(current - 1)
    Keys.onRightPressed: if (current < options.length - 1) picked(current + 1)

    Rectangle {
        id: track
        anchors.fill: parent
        radius: Theme.radiusSm + 2
        color: Theme.bg
        border.width: 1
        border.color: Theme.hairline

        Rectangle {
            id: pill
            y: 3; height: parent.height - 6
            width: root.segW
            radius: Theme.radiusSm
            visible: root.current >= 0
            color: Theme.surfaceHi
            border.width: 1
            border.color: Theme.edgeLight
            x: 3 + Math.max(root.current, 0) * root.segW
            Behavior on x {
                enabled: !Theme.reduce
                SpringAnimation { spring: Theme.snappySpring; damping: Theme.snappyDamping; epsilon: 0.25 }
            }
        }

        Row {
            x: 3; y: 3
            height: parent.height - 6
            Repeater {
                model: root.options
                Item {
                    id: seg
                    required property int index
                    required property string modelData
                    width: root.segW; height: parent.height
                    Text {
                        anchors.centerIn: parent
                        text: seg.modelData
                        font.family: Theme.family
                        font.pixelSize: Theme.sizeBody
                        font.weight: seg.index === root.current ? Font.DemiBold : Font.Medium
                        color: seg.index === root.current ? Theme.accent : (ma.containsMouse ? Theme.text : Theme.text2)
                        Behavior on color { ColorAnimation { duration: Theme.d(Theme.fast) } }
                        scale: ma.pressed ? 0.96 : 1
                        Behavior on scale { NumberAnimation { duration: Theme.instant } }
                    }
                    MouseArea {
                        id: ma
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onPressed: root.forceActiveFocus(Qt.MouseFocusReason)
                        onClicked: root.picked(seg.index)
                    }
                }
            }
        }
    }
    FocusRing { show: root.activeFocus; r: Theme.radiusSm + 2 }
}
