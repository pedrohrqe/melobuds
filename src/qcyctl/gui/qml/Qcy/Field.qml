import QtQuick
import QtQuick.Controls.Basic as C
import Qcy

C.TextField {
    id: root
    implicitHeight: Theme.control
    leftPadding: Theme.s3
    rightPadding: Theme.s3
    font.family: Theme.family
    font.pixelSize: Theme.sizeBody
    color: Theme.text
    placeholderTextColor: Theme.text3
    selectionColor: Theme.accentSoft
    selectedTextColor: Theme.text
    background: Rectangle {
        radius: Theme.radiusSm
        color: Theme.bg
        border.width: root.activeFocus ? 2 : 1
        border.color: root.activeFocus ? Theme.accent : Theme.edgeLight
    }
}
