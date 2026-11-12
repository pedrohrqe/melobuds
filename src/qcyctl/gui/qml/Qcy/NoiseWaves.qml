import QtQuick
import Qcy

// Ondas atrás dos fones que mostram o modo de ruído.
// ANC: as ondas entram e escurecem (o som de fora é absorvido).
// Transparência: as ondas saem e clareiam (o som de fora passa).
// Normal: anéis parados e fracos.
Item {
    id: root
    property int kind: -1             // NoiseKind: 0 normal, 1 ANC, 2 transparência
    property int level: 3
    property bool active: true
    readonly property bool moving: active && !Theme.reduce && (kind === 1 || kind === 2)
    readonly property int period: kind === 2 ? 3400 - level * 220 : 3000

    Repeater {
        model: 3
        Rectangle {
            id: ring
            required property int index
            anchors.centerIn: parent
            width: root.width * 0.62
            height: root.height * 0.78
            radius: height / 2
            color: "transparent"
            border.width: 1
            border.color: root.kind === 2 ? Theme.accent : Theme.text2
            property real phase: index / 3        // 0..1, posição no ciclo
            property real t: 0
            readonly property real p: (t + phase) % 1
            // Escala e opacidade derivadas da fase; ANC inverte a direção.
            scale: !root.moving
                   ? 0.86 + index * 0.16
                   : (root.kind === 2 ? 0.78 + p * 0.62 : 1.4 - p * 0.62)
            opacity: {
                if (!root.active) return 0.05
                if (!root.moving) return root.kind === 0 ? 0.08 : 0.12
                var fade = Math.sin(p * Math.PI)
                return (root.kind === 2 ? 0.5 : 0.26) * fade
            }
            Behavior on opacity { enabled: !root.moving; NumberAnimation { duration: Theme.d(Theme.normal) } }
            Behavior on scale { enabled: !root.moving; NumberAnimation { duration: Theme.d(Theme.slow); easing.type: Easing.OutCubic } }
            NumberAnimation on t {
                running: root.moving
                from: 0; to: 1
                duration: root.period
                loops: Animation.Infinite
            }
        }
    }
    // Brilho central suave no modo transparência.
    Rectangle {
        anchors.centerIn: parent
        width: root.width * 0.5; height: width; radius: width / 2
        color: Theme.accent
        opacity: root.active && root.kind === 2 ? 0.05 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.d(Theme.slow) } }
    }
}
