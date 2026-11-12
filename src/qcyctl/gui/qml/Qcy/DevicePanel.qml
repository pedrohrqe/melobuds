import QtQuick
import Qcy

// Dispositivo: modos, nome Bluetooth, áudio e informações.
Column {
    id: root
    width: parent ? parent.width : 300
    spacing: Theme.s1

    Toggle {
        text: "Modo jogo"
        hint: "Reduz o atraso do áudio. Pode diminuir o alcance."
        checked: backend.gameMode
        onToggled: (on) => backend.setGame(on)
    }
    Toggle {
        text: "Modo sono"
        hint: "Desativa os toques até você guardar os fones na caixa."
        checked: backend.sleepMode
        onToggled: (on) => backend.setSleep(on)
    }

    Item { width: 1; height: Theme.s2 }
    FieldLabel { text: "Nome Bluetooth" }
    Item { width: 1; height: Theme.s1 }
    Row {
        width: parent.width
        spacing: Theme.s2
        Field {
            id: nameField
            width: parent.width - save.width - Theme.s2
            text: backend.name
            maximumLength: 32
            placeholderText: "Nome do fone"
            onAccepted: save.clicked()
        }
        Button {
            id: save
            text: "Salvar"
            primary: true
            enabled: nameField.text.trim() !== "" && nameField.text !== backend.name
            onClicked: if (backend.rename(nameField.text)) nameField.focus = false
        }
    }

    Item { width: 1; height: Theme.s3 }
    Row {
        width: parent.width
        spacing: Theme.s3
        Button {
            text: "Conectar áudio"
            onClicked: backend.connectAudio()
        }
        Text {
            width: parent.width - parent.children[0].width - Theme.s3
            anchors.verticalCenter: parent.verticalCenter
            wrapMode: Text.WordWrap
            text: "Use se o som não sair pelo fone."
            font.family: Theme.family; font.pixelSize: Theme.sizeLabel + 1
            color: Theme.text2
        }
    }

    Item { width: 1; height: Theme.s3 }
    Toggle {
        text: "Reduzir animações"
        hint: "Troca os movimentos por transições curtas."
        checked: backend.reduceMotion
        onToggled: (on) => backend.reduceMotion = on
    }
    Toggle {
        text: "Ignorar caixa"
        hint: "Esconde a caixa e descarta as informações dela."
        checked: backend.ignoreCase
        onToggled: (on) => backend.ignoreCase = on
    }
    Toggle {
        text: "Iniciar com o sistema"
        hint: "Abre o app na bandeja quando você entra no sistema."
        checked: backend.autostart
        onToggled: (on) => backend.autostart = on
    }
    Item { width: 1; height: Theme.s2 }
    Rectangle { width: parent.width; height: 1; color: Theme.hairline }
    InfoRow { label: "Firmware"; value: backend.firmware }
    InfoRow { label: "Endereço"; value: backend.address }
}
