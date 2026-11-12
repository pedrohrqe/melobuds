import QtQuick
import Qcy

// Anel de foco do teclado. Coloque como filho do controle.
Rectangle {
    property bool show: parent.activeFocus
    property real r: Theme.radiusSm
    anchors.fill: parent
    anchors.margins: -3
    radius: r + 3
    color: "transparent"
    border.width: 2
    border.color: Theme.accent
    visible: show
}
