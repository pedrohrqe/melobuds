import QtQuick
import Qcy

// A caixa de carga: cápsula com a junta da tampa e o LED frontal.
Item {
    id: root
    property bool present: true
    property bool charging: false
    implicitWidth: 104
    implicitHeight: 72

    Rectangle {
        id: shell
        anchors.fill: parent
        radius: height / 2.2
        opacity: root.present ? 1 : 0.55
        border.width: 1
        border.color: root.present ? Theme.edgeLight : Theme.text3
        gradient: Gradient {
            GradientStop { position: 0; color: root.present ? "#30353E" : "transparent" }
            GradientStop { position: 1; color: root.present ? Theme.budBottom : "transparent" }
        }
        Behavior on opacity { NumberAnimation { duration: Theme.d(Theme.normal) } }

        // Junta da tampa
        Rectangle {
            x: 10; width: parent.width - 20; height: 1
            y: parent.height * 0.38
            color: "#99000000"
        }
        Rectangle {
            x: 12; width: parent.width - 24; height: 1
            y: parent.height * 0.38 + 1
            color: Theme.hairline
        }
        // LED frontal
        Rectangle {
            id: led
            width: 6; height: 6; radius: 3
            anchors.horizontalCenter: parent.horizontalCenter
            y: parent.height * 0.62
            color: root.charging ? Theme.accent : "#2A2F36"
            SequentialAnimation on opacity {
                running: root.charging && !Theme.reduce
                loops: Animation.Infinite
                alwaysRunToEnd: true
                NumberAnimation { to: 0.35; duration: 1400; easing.type: Easing.InOutSine }
                NumberAnimation { to: 1; duration: 1400; easing.type: Easing.InOutSine }
            }
        }
        Rectangle {
            anchors.centerIn: led
            width: 22; height: 22; radius: 11
            color: Theme.accent
            opacity: root.charging ? led.opacity * 0.18 : 0
        }
    }
}
