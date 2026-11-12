import QtQuick
import Qcy

// Opção em pílula (presets de EQ, cenas de ANC).
Rectangle {
    id: root
    property string text: ""
    property bool checked: false
    signal clicked()
    implicitWidth: label.implicitWidth + 2 * Theme.s3
    implicitHeight: 30
    radius: height / 2
    color: checked ? Theme.accentSoft : (ma.containsMouse ? Theme.surfaceHi : "transparent")
    border.width: 1
    border.color: checked ? Theme.accent : Theme.hairline
    scale: ma.pressed ? 0.97 : 1
    activeFocusOnTab: true
    Keys.onSpacePressed: clicked()
    Keys.onReturnPressed: clicked()
    Behavior on color { ColorAnimation { duration: Theme.d(Theme.fast) } }
    Behavior on border.color { ColorAnimation { duration: Theme.d(Theme.fast) } }
    Behavior on scale { NumberAnimation { duration: Theme.instant } }

    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        font.family: Theme.family
        font.pixelSize: Theme.sizeBody
        font.weight: root.checked ? Font.DemiBold : Font.Medium
        color: root.checked ? Theme.accent : Theme.text2
    }
    MouseArea {
        id: ma
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
    FocusRing { show: root.activeFocus; r: root.radius - 3 }
}
