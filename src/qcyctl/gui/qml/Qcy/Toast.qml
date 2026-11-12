import QtQuick
import Qcy

// Aviso no rodapé. Entra de baixo e sai pelo mesmo caminho.
Rectangle {
    id: root
    property string kind: "info"
    property bool shown: false
    function show(text, k) { msg.text = text; kind = k; shown = true; timer.restart() }

    width: Math.min(parent.width - 2 * Theme.s5, 380)
    height: row.implicitHeight + 2 * Theme.s3
    anchors.horizontalCenter: parent.horizontalCenter
    y: shown ? parent.height - height - Theme.s5 : parent.height + 8
    opacity: shown ? 1 : 0
    radius: Theme.radius
    color: Theme.surfaceHi
    border.width: 1
    border.color: kind === "error" ? Theme.bad : Theme.edgeLight
    Behavior on y { enabled: !Theme.reduce; NumberAnimation { duration: Theme.normal; easing.type: Easing.OutCubic } }
    Behavior on opacity { NumberAnimation { duration: Theme.d(Theme.fast) } }

    Timer { id: timer; interval: root.kind === "error" ? 6000 : 4000; onTriggered: root.shown = false }

    Row {
        id: row
        x: Theme.s4; anchors.verticalCenter: parent.verticalCenter
        width: parent.width - 2 * Theme.s4
        spacing: Theme.s3
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: root.kind === "error" ? "alert" : "check"
            color: root.kind === "error" ? Theme.bad : Theme.accent
        }
        Text {
            id: msg
            width: parent.width - 18 - Theme.s3
            wrapMode: Text.WordWrap
            font.family: Theme.family; font.pixelSize: Theme.sizeBody
            color: Theme.text
        }
    }
    MouseArea { anchors.fill: parent; onClicked: root.shown = false }
}
