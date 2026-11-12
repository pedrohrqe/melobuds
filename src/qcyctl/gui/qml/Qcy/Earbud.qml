import QtQuick
import QtQuick.Shapes
import Qcy

// Um fone HT05 de frente: cabeça arredondada, ponta de silicone e haste.
// Desenhado para o lado esquerdo; o direito é espelhado.
Item {
    id: root
    property bool mirrored: false
    property bool present: true       // false: só o contorno (procurando)
    implicitWidth: 92
    implicitHeight: 132

    Shape {
        id: body
        width: 92; height: 132
        anchors.centerIn: parent
        preferredRendererType: Shape.CurveRenderer
        transform: Scale { origin.x: 46; xScale: root.mirrored ? -1 : 1 }
        opacity: root.present ? 1 : 0.55
        Behavior on opacity { NumberAnimation { duration: Theme.d(Theme.normal); easing.type: Easing.OutCubic } }

        // Haste
        ShapePath {
            strokeColor: Theme.edgeLight
            strokeWidth: 1
            fillGradient: LinearGradient {
                x1: 26; y1: 0; x2: 52; y2: 0
                GradientStop { position: 0; color: root.present ? Theme.budBottom : "transparent" }
                GradientStop { position: 0.45; color: root.present ? Theme.budTop : "transparent" }
                GradientStop { position: 1; color: root.present ? Theme.budBottom : "transparent" }
            }
            PathSvg { path: "M28 52 L28 118 Q28 130 40 130 Q52 130 52 118 L52 52 Z" }
        }
        // Cabeça
        ShapePath {
            strokeColor: root.present ? Theme.edgeLight : Theme.text3
            strokeWidth: 1
            fillGradient: RadialGradient {
                centerX: 36; centerY: 24; centerRadius: 52
                focalX: 32; focalY: 18
                GradientStop { position: 0; color: root.present ? "#3A404A" : "transparent" }
                GradientStop { position: 1; color: root.present ? Theme.budBottom : "transparent" }
            }
            PathSvg { path: "M44 6 C68 6 80 22 80 40 C80 58 64 70 44 70 C22 70 8 58 8 40 C8 20 22 6 44 6 Z" }
        }
        // Ponta de silicone
        ShapePath {
            strokeColor: "transparent"
            fillColor: root.present ? Theme.tip : "transparent"
            PathSvg { path: "M74 24 C86 22 92 30 92 38 C92 47 85 53 75 51 C79 42 79 32 74 24 Z" }
        }
        // Brilho da cabeça (luz de cima)
        ShapePath {
            strokeColor: "transparent"
            fillColor: root.present ? "#0FFFFFFF" : "transparent"
            PathSvg { path: "M26 16 C34 10 50 9 60 14 C50 13 36 15 26 22 Z" }
        }
        // Microfone na ponta da haste
        ShapePath {
            strokeColor: "transparent"
            fillColor: root.present ? "#80000000" : "transparent"
            PathSvg { path: "M36 122 a4 2 0 1 0 8 0 a4 2 0 1 0 -8 0 Z" }
        }
    }
}
