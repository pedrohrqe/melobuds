import QtQuick
import Qcy

// Cartão com cabeçalho. Expansível: a altura anima e o conteúdo aparece com fade.
Item {
    id: root
    property string title: ""
    property string value: ""         // resumo à direita do título
    property bool expandable: true
    property bool expanded: !expandable
    property Item headerExtra: null
    default property alias content: body.data
    signal toggled()

    // Altura com o cartão aberto. A janela reserva espaço para o maior cartão.
    readonly property real headerHeight: header.height
    readonly property real openHeight: header.height + body.childrenRect.height + Theme.s4
    implicitHeight: header.height + (expanded ? body.childrenRect.height + Theme.s4 : 0)
    clip: true
    Behavior on implicitHeight {
        NumberAnimation { duration: Theme.d(Theme.normal); easing.type: Easing.OutCubic }
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.radius
        color: Theme.surface
        border.width: 1
        border.color: Theme.hairline
        // Luz na borda superior do material.
        Rectangle {
            x: Theme.radius; width: parent.width - 2 * Theme.radius
            y: 0; height: 1
            color: Theme.edgeLight
        }
    }

    Item {
        id: header
        width: parent.width
        height: 52
        Text {
            anchors.left: parent.left
            anchors.leftMargin: Theme.s4
            anchors.verticalCenter: parent.verticalCenter
            text: root.title
            font.family: Theme.family
            font.pixelSize: Theme.sizeHeading
            font.weight: Font.DemiBold
            color: Theme.text
        }
        Row {
            anchors.right: parent.right
            anchors.rightMargin: Theme.s4
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.s2
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.value
                font.family: Theme.family
                font.pixelSize: Theme.sizeBody
                color: Theme.text2
                elide: Text.ElideRight
            }
            Icon {
                visible: root.expandable
                anchors.verticalCenter: parent.verticalCenter
                name: "chevron"
                rotation: root.expanded ? 180 : 0
                Behavior on rotation { NumberAnimation { duration: Theme.d(Theme.normal); easing.type: Easing.OutCubic } }
            }
        }
        MouseArea {
            id: hma
            anchors.fill: parent
            enabled: root.expandable
            hoverEnabled: true
            cursorShape: root.expandable ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: { header.forceActiveFocus(Qt.MouseFocusReason); root.toggled() }
        }
        activeFocusOnTab: root.expandable
        Keys.onSpacePressed: root.toggled()
        Keys.onReturnPressed: root.toggled()
        Rectangle {
            anchors.fill: parent; anchors.margins: 2
            radius: Theme.radius - 2
            color: "transparent"
            border.width: 2; border.color: Theme.accent
            visible: header.activeFocus
        }
    }

    Item {
        id: body
        x: Theme.s4
        y: header.height
        width: parent.width - 2 * Theme.s4
        height: childrenRect.height
        opacity: root.expanded ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.d(root.expanded ? Theme.normal : Theme.fast) } }
    }
}
