import QtQuick
import Qcy

// Toques: uma ação para cada gesto em cada lado.
Column {
    id: root
    width: parent ? parent.width : 300
    spacing: Theme.s3
    readonly property var rows: [["Toque", 1, 2], ["Toque duplo", 3, 4], ["Toque triplo", 5, 6]]
    readonly property real colW: (width - Theme.s2) / 2

    Repeater {
        model: root.rows
        Column {
            id: row
            required property var modelData
            width: root.width
            spacing: Theme.s1 + 2
            Text {
                text: row.modelData[0]
                font.family: Theme.family; font.pixelSize: Theme.sizeBody; font.weight: Font.Medium
                color: Theme.text
            }
            Row {
                spacing: Theme.s2
                Repeater {
                    model: [row.modelData[1], row.modelData[2]]
                    Column {
                        id: side
                        required property int index
                        required property int modelData
                        width: root.colW
                        spacing: 2
                        Select {
                            id: sel
                            width: parent.width
                            model: backend.keyActions
                            textRole: "label"
                            valueRole: "value"
                            prefix: side.index === 0 ? "E" : "D"
                            readonly property int action: backend.keys[side.modelData - 1]
                            // A ativação pelo usuário quebra bindings de currentIndex; sincroniza por evento.
                            onActionChanged: currentIndex = indexOfValue(action)
                            Component.onCompleted: currentIndex = indexOfValue(action)
                            onActivated: backend.setKey(side.modelData, currentValue)
                        }
                    }
                }
            }
        }
    }
    Text {
        width: parent.width
        wrapMode: Text.WordWrap
        text: "E = fone esquerdo, D = fone direito. Em chamadas, os gestos são fixos: toque duplo atende ou desliga; segurar 1,5 s recusa."
        font.family: Theme.family; font.pixelSize: Theme.sizeLabel + 1
        color: Theme.text2
    }
}
