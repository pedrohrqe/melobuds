import QtQuick
import Qcy

// Linha com texto, descrição e interruptor. O botão desliza com mola.
Item {
    id: root
    property string text: ""
    property string hint: ""
    property bool checked: false
    signal toggled(bool on)
    width: parent ? parent.width : 300
    implicitHeight: Math.max(44, col.implicitHeight + Theme.s3)
    activeFocusOnTab: true
    Keys.onSpacePressed: toggled(!checked)

    Column {
        id: col
        anchors.left: parent.left
        anchors.right: sw.left
        anchors.rightMargin: Theme.s3
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        Text {
            text: root.text
            font.family: Theme.family; font.pixelSize: Theme.sizeBody; font.weight: Font.Medium
            color: Theme.text
        }
        Text {
            visible: root.hint !== ""
            width: parent.width
            text: root.hint
            wrapMode: Text.WordWrap
            font.family: Theme.family; font.pixelSize: Theme.sizeLabel + 1
            color: Theme.text2
        }
    }
    Rectangle {
        id: sw
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: 40; height: 24; radius: 12
        color: root.checked ? Theme.accent : Theme.surfaceHi
        border.width: 1
        border.color: root.checked ? Theme.accent : Theme.edgeLight
        Behavior on color { ColorAnimation { duration: Theme.d(Theme.fast) } }
        Rectangle {
            width: 18; height: 18; radius: 9
            y: 3
            x: root.checked ? parent.width - width - 3 : 3
            color: root.checked ? Theme.accentInk : Theme.text2
            Behavior on x { enabled: !Theme.reduce; SpringAnimation { spring: Theme.snappySpring; damping: Theme.snappyDamping; epsilon: 0.25 } }
        }
        FocusRing { show: root.activeFocus; r: 12 }
    }
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled(!root.checked)
    }
}
