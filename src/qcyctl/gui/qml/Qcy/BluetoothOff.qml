import QtQuick
import Qcy

// Aviso de Bluetooth desligado. O emblema mostra o estado sem depender só da cor:
// um traço corta o símbolo do Bluetooth. Ao ligar, o traço recolhe e um anel pulsa uma vez.
Rectangle {
    id: root
    property bool off: false                 // Bluetooth do sistema desligado
    property bool canEnable: false           // o app consegue ligar o Bluetooth
    signal enableRequested()

    implicitHeight: col.implicitHeight + 2 * Theme.s5
    height: implicitHeight
    radius: Theme.radius
    color: Theme.surface
    border.width: 1
    border.color: Theme.hairline
    visible: opacity > 0
    opacity: 0
    // Luz na borda superior do material.
    Rectangle { x: Theme.radius; width: parent.width - 2 * Theme.radius; height: 1; color: Theme.edgeLight }

    property bool busy: false
    onOffChanged: {
        busy = false
        if (off) { hide.stop(); show.restart() } else { show.stop(); hide.restart() }
    }
    Component.onCompleted: if (off) opacity = 1
    Timer { id: busyReset; interval: 5000; onTriggered: root.busy = false }

    Column {
        id: col
        x: Theme.s5; y: Theme.s5
        width: parent.width - 2 * Theme.s5
        spacing: Theme.s4

        Item {
            id: emblem
            width: 64; height: 64
            transform: Translate { id: emblemY }

            // Anel de confirmação: uma vez, quando o Bluetooth liga.
            Rectangle {
                id: ring
                anchors.centerIn: parent
                width: 64; height: 64; radius: 32
                color: "transparent"
                border.width: 2; border.color: Theme.accent
                opacity: 0
            }
            Rectangle {
                anchors.fill: parent
                radius: 32
                color: Theme.surfaceHi
                border.width: 1
                border.color: root.off ? Theme.edgeLight : Theme.accent
                Behavior on border.color { ColorAnimation { duration: Theme.d(Theme.fast) } }
            }
            Icon {
                anchors.centerIn: parent
                width: 28; height: 28
                name: "bluetooth"
                color: root.off ? Theme.text3 : Theme.accent
            }
            // Traço de "desligado": cresce a partir do canto superior esquerdo.
            Item {
                anchors.centerIn: parent
                width: 44; height: 44
                rotation: 45
                Rectangle {                 // contorno: separa o traço do símbolo
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width; height: 6; radius: 3
                    color: Theme.surfaceHi
                    transform: Scale { xScale: slash.xScale }
                }
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width; height: 2; radius: 1
                    color: Theme.bad
                    transform: Scale {
                        id: slash
                        xScale: root.off ? 1 : 0
                        Behavior on xScale {
                            NumberAnimation { duration: Theme.d(root.off ? Theme.normal : Theme.fast); easing.type: Easing.OutCubic }
                        }
                    }
                }
            }
        }

        Column {
            width: parent.width
            spacing: Theme.s2
            Text {
                text: root.off ? "Bluetooth desligado" : "Bluetooth ligado"
                font.family: Theme.family; font.pixelSize: Theme.sizeHeading; font.weight: Font.DemiBold
                color: Theme.text
            }
            Text {
                width: parent.width
                wrapMode: Text.WordWrap
                lineHeight: 1.3
                text: root.canEnable
                      ? "O app não procura o fone com o Bluetooth desligado. Ligue o Bluetooth; o app conecta o fone em seguida."
                      : "O app não procura o fone com o Bluetooth desligado. Ligue o Bluetooth nas configurações do sistema; o app conecta o fone em seguida."
                font.family: Theme.family; font.pixelSize: Theme.sizeBody
                color: Theme.text2
            }
        }

        Button {
            visible: root.canEnable
            primary: true
            enabled: root.off && !root.busy
            text: root.busy ? "Ligando…" : "Ligar Bluetooth"
            onClicked: { root.busy = true; busyReset.restart(); root.enableRequested() }
        }
    }

    // Entrada: fade do cartão; o emblema sobe 16 px.
    ParallelAnimation {
        id: show
        NumberAnimation { target: root; property: "opacity"; to: 1; duration: Theme.d(Theme.normal); easing.type: Easing.OutCubic }
        NumberAnimation { target: emblemY; property: "y"; from: Theme.reduce ? 0 : 16; to: 0; duration: Theme.d(Theme.normal); easing.type: Easing.OutCubic }
    }
    // Saída: o anel confirma "ligado" uma vez, e só depois o cartão some.
    SequentialAnimation {
        id: hide
        ParallelAnimation {
            NumberAnimation { target: ring; property: "scale"; from: 1; to: 1.7; duration: Theme.reduce ? 0 : Theme.slow; easing.type: Easing.OutCubic }
            NumberAnimation { target: ring; property: "opacity"; from: Theme.reduce ? 0 : 0.6; to: 0; duration: Theme.reduce ? 0 : Theme.slow; easing.type: Easing.OutCubic }
        }
        NumberAnimation { target: root; property: "opacity"; to: 0; duration: Theme.d(Theme.fast); easing.type: Easing.OutCubic }
    }
}
