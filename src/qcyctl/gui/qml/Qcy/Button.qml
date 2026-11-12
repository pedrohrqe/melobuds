import QtQuick
import Qcy

// Botão. "primary" usa o acento; o padrão é neutro.
Rectangle {
    id: root
    property string text: ""
    property bool primary: false
    signal clicked()
    implicitWidth: label.implicitWidth + 2 * Theme.s4
    implicitHeight: Theme.control
    radius: Theme.radiusSm
    opacity: enabled ? 1 : 0.4
    color: primary ? (ma.pressed ? Qt.darker(Theme.accent, 1.15) : Theme.accent)
                   : (ma.pressed ? Theme.surfacePress : (ma.containsMouse ? Theme.surfaceHi : Theme.bg))
    border.width: primary ? 0 : 1
    border.color: Theme.edgeLight
    scale: ma.pressed ? 0.97 : 1
    activeFocusOnTab: true
    Keys.onSpacePressed: clicked()
    Keys.onReturnPressed: clicked()
    Behavior on scale { NumberAnimation { duration: Theme.instant } }
    Behavior on color { ColorAnimation { duration: Theme.d(Theme.fast) } }

    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        font.family: Theme.family; font.pixelSize: Theme.sizeBody; font.weight: Font.DemiBold
        color: root.primary ? Theme.accentInk : Theme.text
    }
    MouseArea {
        id: ma
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
    FocusRing { show: root.activeFocus; r: Theme.radiusSm }
}
