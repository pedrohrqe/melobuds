import QtQuick
import Qcy

// Modo de ruído: Normal, ANC (cena + intensidade) ou Transparência (nível ou voz).
Card {
    id: card
    title: "Ruído"
    expandable: false
    value: backend.noiseLabel

    Column {
        width: parent.width
        spacing: Theme.s3

        SegmentedControl {
            width: parent.width
            options: ["Normal", "ANC", "Transparência"]
            current: backend.noiseKind
            onPicked: (i) => {
                if (i === 1) backend.setNoise(1, backend.ancScene, backend.ancLevel)
                else if (i === 2) backend.setNoise(2, "", backend.transLevel)
                else backend.setNoise(0, "", 0)
            }
        }

        // ANC
        Column {
            width: parent.width
            spacing: Theme.s2
            visible: backend.noiseKind === 1
            FieldLabel { text: "Ambiente" }
            Flow {
                width: parent.width
                spacing: Theme.s2
                Repeater {
                    model: backend.ancScenes
                    Chip {
                        required property var modelData
                        text: modelData.label
                        checked: backend.ancScene === modelData.key
                        onClicked: backend.setNoise(1, modelData.key, backend.ancLevel)
                    }
                }
            }
            Item { width: 1; height: Theme.s1 }
            FieldLabel { text: "Intensidade"; visible: backend.ancScene !== "wind" }
            SegmentedControl {
                visible: backend.ancScene !== "wind"
                width: parent.width
                options: ["Leve", "Média", "Forte"]
                current: backend.ancLevel - 1
                onPicked: (i) => backend.setNoise(1, backend.ancScene, i + 1)
            }
        }

        // Transparência
        Column {
            width: parent.width
            spacing: Theme.s2
            visible: backend.noiseKind === 2
            Row {
                width: parent.width
                FieldLabel { text: "Nível"; width: parent.width - lvl.width }
                Text {
                    id: lvl
                    text: backend.transLevel === 0 ? "Voz" : backend.transLevel + " de 6"
                    font.family: Theme.family; font.pixelSize: Theme.sizeLabel + 1
                    font.features: { "tnum": 1 }
                    color: Theme.text
                }
            }
            StepSlider {
                width: parent.width
                from: 1; to: 6
                enabled: backend.transLevel !== 0
                opacity: enabled ? 1 : 0.4
                value: backend.transLevel === 0 ? 4 : backend.transLevel
                onMoved: (v) => backend.setNoise(2, "", v)
            }
            Toggle {
                width: parent.width
                text: "Realçar voz"
                hint: "Destaca a fala de quem está perto e reduz o resto."
                checked: backend.transLevel === 0
                onToggled: (on) => backend.setNoise(2, "", on ? 0 : 4)
            }
        }
    }
}
