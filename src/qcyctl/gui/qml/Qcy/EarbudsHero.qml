import QtQuick
import Qcy

// Herói: fones esquerdo e direito e a caixa, cada um com a bateria acima.
Item {
    id: root
    property bool connected: false
    property var battery: [{percent: -1, charging: false}, {percent: -1, charging: false}, {percent: -1, charging: false}]
    property int noiseKind: -1
    property int noiseLevel: 3
    property bool showCase: true             // "Ignorar caixa" esconde a caixa e centraliza os fones
    property real ox: showCase ? 0 : colW / 2
    Behavior on ox { NumberAnimation { duration: Theme.d(Theme.normal); easing.type: Easing.OutCubic } }
    implicitHeight: 236

    function sweep() { left.sweep(); rightR.sweep(); caseR.sweep() }

    readonly property real colW: width / 3

    NoiseWaves {
        x: root.ox; y: 54
        width: root.colW * 2; height: 172
        kind: root.noiseKind
        level: root.noiseLevel
        active: root.connected
    }

    // Anel de busca: pulsa enquanto o app procura o fone.
    Rectangle {
        id: searchRing
        x: root.ox + root.colW - width / 2; y: 140 - height / 2
        width: 190; height: 150; radius: 75
        color: "transparent"
        border.color: Theme.accent
        border.width: 1
        visible: !root.connected && !Theme.reduce
        SequentialAnimation on scale {
            running: searchRing.visible
            loops: Animation.Infinite
            NumberAnimation { from: 0.7; to: 1.15; duration: 1800; easing.type: Easing.OutCubic }
        }
        SequentialAnimation on opacity {
            running: searchRing.visible
            loops: Animation.Infinite
            NumberAnimation { from: 0.45; to: 0; duration: 1800; easing.type: Easing.OutCubic }
        }
    }

    BatteryReadout {
        id: left
        x: root.ox + root.colW * 0.5 - width / 2 + 10; y: 0
        label: "Esquerdo"
        percent: root.connected ? root.battery[0].percent : -1
        charging: root.connected && root.battery[0].charging
    }
    BatteryReadout {
        id: rightR
        x: root.ox + root.colW * 1.5 - width / 2 - 10; y: 0
        label: "Direito"
        percent: root.connected ? root.battery[1].percent : -1
        charging: root.connected && root.battery[1].charging
    }
    BatteryReadout {
        id: caseR
        x: root.colW * 2.5 - width / 2; y: 0
        label: "Caixa"
        visible: opacity > 0
        opacity: root.showCase ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.d(Theme.fast) } }
        percent: root.connected ? root.battery[2].percent : -1
        charging: root.connected && root.battery[2].charging
    }

    Earbud {
        x: root.ox + root.colW * 0.5 - width / 2 + 6; y: 82
        present: root.connected
    }
    Earbud {
        x: root.ox + root.colW * 1.5 - width / 2 - 6; y: 82
        mirrored: true
        present: root.connected
    }
    CaseShape {
        x: root.colW * 2.5 - width / 2; y: 120
        present: root.connected && root.battery[2].percent >= 0
        visible: opacity > 0
        opacity: root.showCase ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.d(Theme.fast) } }
        charging: root.connected && root.battery[2].charging
    }
}
