import QtQuick
import QtQuick.Controls.Basic as C
import Qcy

Window {
    id: win
    width: 940
    height: 700
    minimumWidth: 940
    visible: false
    color: Theme.bg
    title: backend.appName

    property bool quitting: false
    property string openCard: ""
    property bool entered: false

    onClosing: (close) => {
        if (backend.trayAvailable && !quitting) { close.accepted = false; win.hide() }
    }

    Binding { target: Theme; property: "reduce"; value: backend.reduceMotion }

    FontLoader { source: "Qcy/fonts/Inter-Regular.ttf" }
    FontLoader { source: "Qcy/fonts/Inter-Medium.ttf" }
    FontLoader { source: "Qcy/fonts/Inter-SemiBold.ttf" }
    FontLoader { source: "Qcy/fonts/InterDisplay-SemiBold.ttf" }

    Connections {
        target: backend
        function onToast(text, kind) { toast.show(text, kind) }
        function onChanged() {
            if (backend.connected && !hero.wasConnected) { hero.wasConnected = true; sweepDelay.restart() }
            else if (!backend.connected) hero.wasConnected = false
        }
    }
    Timer { id: sweepDelay; interval: 250; onTriggered: hero.sweep() }

    // Sem rolagem: duas colunas. A altura da janela acompanha o conteúdo e
    // reserva o espaço do maior painel, para não mudar ao trocar de aba.
    readonly property real colGap: Theme.s5
    property real leftPeak: 0
    readonly property var tabKeys: ["eq", "keys", "device"]
    readonly property int tabIndex: Math.max(0, tabKeys.indexOf(openCard))
    readonly property real rightNeed: backend.connected
        ? tabs.height + Theme.s3 + 2 * Theme.s4
          + Math.max(eqPanel.chrome + 168, keysPanel.implicitHeight, devPanel.implicitHeight)
        : (backend.bluetoothOn ? guide.implicitHeight : btOff.implicitHeight)
    readonly property real contentNeed: Math.max(leftPeak + credit.height + Theme.s3, rightNeed)
    onContentNeedChanged: fit()
    function fit() {
        const h = Math.ceil(contentNeed + 2 * Theme.s5)
        minimumHeight = h; maximumHeight = h; height = h
    }
    Connections {
        target: left
        function onImplicitHeightChanged() { win.leftPeak = Math.max(win.leftPeak, left.implicitHeight) }
    }
    Component.onCompleted: { leftPeak = left.implicitHeight; fit() }

    Row {
        x: Theme.s5; y: Theme.s5
        spacing: win.colGap
        readonly property real leftW: 400
        readonly property real rightW: win.width - 2 * Theme.s5 - win.colGap - leftW

        Column {
            id: left
            width: parent.leftW
            spacing: Theme.s3

            // Cabeçalho: nome do fone e estado.
            Item {
                width: parent.width
                height: 58
                Text {
                    id: title
                    width: parent.width
                    text: backend.connected ? backend.name : "QCY HT05"
                    elide: Text.ElideRight
                    font.family: Theme.display
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.sizeTitle
                    font.letterSpacing: -0.4
                    color: Theme.text
                }
                Row {
                    anchors.top: title.bottom
                    anchors.topMargin: 4
                    spacing: Theme.s2
                    Rectangle {
                        id: dot
                        width: 8; height: 8; radius: 4
                        anchors.verticalCenter: parent.verticalCenter
                        color: !backend.bluetoothOn ? Theme.bad : (backend.connected ? Theme.ok : Theme.warn)
                        Behavior on color { ColorAnimation { duration: Theme.d(Theme.normal) } }
                        SequentialAnimation on opacity {
                            running: !backend.connected && backend.bluetoothOn && !Theme.reduce
                            loops: Animation.Infinite
                            alwaysRunToEnd: true
                            NumberAnimation { to: 0.3; duration: 700; easing.type: Easing.InOutSine }
                            NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
                        }
                    }
                    Text {
                        text: backend.status
                        width: left.width - 16
                        elide: Text.ElideRight
                        font.family: Theme.family
                        font.pixelSize: Theme.sizeBody
                        color: Theme.text2
                    }
                }
            }


            EarbudsHero {
                id: hero
                property bool wasConnected: false
                width: parent.width
                connected: backend.connected
                showCase: !backend.ignoreCase
                battery: backend.battery
                noiseKind: backend.connected ? backend.noiseKind : -1
                noiseLevel: backend.noiseKind === 1 ? backend.ancLevel : backend.transLevel
            }


            Item { width: 1; height: Theme.s1 }

            NoisePanel { id: c0; width: parent.width; visible: backend.connected; transform: Translate { id: t0 } }
        }

        Item {
            width: parent.rightW
            height: win.contentNeed

            // Desconectado: instruções.
            Rectangle {
                id: guide
                width: parent.width
                visible: opacity > 0
                // Espera o aviso de Bluetooth sair, para os dois textos não se sobreporem.
                opacity: backend.connected || btOff.visible ? 0 : 1
                implicitHeight: guideCol.implicitHeight + 2 * Theme.s4
            height: implicitHeight
                Behavior on opacity { NumberAnimation { duration: Theme.d(Theme.normal) } }
                radius: Theme.radius
                color: Theme.surface
                border.width: 1
                border.color: Theme.hairline
                Column {
                    id: guideCol
                    x: Theme.s4; y: Theme.s4
                    width: parent.width - 2 * Theme.s4
                    spacing: Theme.s3
                    Text {
                        text: backend.audioOnly ? "Fone conectado ao sistema" : "Conectando ao fone"
                        font.family: Theme.family; font.pixelSize: Theme.sizeHeading; font.weight: Font.DemiBold
                        color: Theme.text
                    }
                    Repeater {
                        model: backend.audioOnly ? [
                            "O áudio funciona. O fone só libera os controles por cerca de 1 minuto depois de conectar.",
                            "Para usar os controles agora, ative-os abaixo. O Bluetooth do computador reinicia por alguns segundos.",
                            "Ou guarde os fones na caixa e tire de novo com este app aberto."
                        ] : [
                            "Tire os fones da caixa e coloque nos ouvidos.",
                            "Deixe este app aberto: ele conecta os controles e o áudio.",
                            "Primeira vez? Coloque o fone em modo de pareamento (veja o manual do fone) e deixe esta janela aberta."
                        ]
                        Row {
                            required property int index
                            required property string modelData
                            spacing: Theme.s3
                            width: guideCol.width
                            Rectangle {
                                width: 22; height: 22; radius: 11
                                color: Theme.surfaceHi
                                border.width: 1; border.color: Theme.edgeLight
                                Text {
                                    anchors.centerIn: parent
                                    text: index + 1
                                    font.family: Theme.family; font.pixelSize: Theme.sizeLabel; font.weight: Font.DemiBold
                                    color: Theme.accent
                                }
                            }
                            Text {
                                width: parent.width - 22 - Theme.s3
                                wrapMode: Text.WordWrap
                                text: modelData
                                lineHeight: 1.3
                                font.family: Theme.family; font.pixelSize: Theme.sizeBody
                                color: Theme.text2
                            }
                        }
                    }
                    Button {
                        visible: backend.audioOnly
                        text: "Ativar controles"
                        primary: true
                        onClicked: backend.wakeControl()
                    }
                }
            }


            // Bluetooth do sistema desligado: aviso no lugar das instruções.
            BluetoothOff {
                id: btOff
                width: parent.width
                off: !backend.bluetoothOn
                canEnable: backend.canEnableBluetooth
                onEnableRequested: backend.enableBluetooth()
            }

            // Conectado: abas e um painel único, sempre aberto.
            SegmentedControl {
                id: tabs
                width: parent.width
                visible: backend.connected
                transform: Translate { id: t1 }
                options: ["Equalizador", "Toques", "Dispositivo"]
                current: win.tabIndex
                onPicked: (i) => win.openCard = win.tabKeys[i]
            }
            Rectangle {
                id: panel
                y: tabs.height + Theme.s3
                width: parent.width
                height: parent.height - y
                visible: backend.connected
                transform: Translate { id: t2 }
                radius: Theme.radius
                color: Theme.surface
                border.width: 1
                border.color: Theme.hairline
                clip: true
                // Luz na borda superior do material.
                Rectangle {
                    x: Theme.radius; width: parent.width - 2 * Theme.radius
                    height: 1
                    color: Theme.edgeLight
                }

                Pane { index: 0; EqPanel { id: eqPanel; width: parent.width; plotH: Math.max(112, parent.height - chrome) } }
                Pane { index: 1; KeysPanel { id: keysPanel; width: parent.width } }
                Pane { index: 2; DevicePanel { id: devPanel; width: parent.width } }

                onVisibleChanged: if (visible && !win.entered) { win.entered = true; if (!Theme.reduce) enter.restart() }
            }
        }
    }

    // Painel de uma aba: entra com fade e um deslize curto na direção da troca.
    component Pane: Item {
        property int index: 0
        readonly property bool active: win.tabIndex === index
        x: Theme.s4; y: Theme.s4
        width: parent.width - 2 * Theme.s4
        height: parent.height - 2 * Theme.s4
        visible: opacity > 0
        enabled: active
        opacity: active ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.d(active ? Theme.normal : Theme.fast); easing.type: Easing.OutCubic } }
        transform: Translate {
            x: Theme.reduce || active ? 0 : (index < win.tabIndex ? -24 : 24)
            Behavior on x { NumberAnimation { duration: Theme.d(Theme.normal); easing.type: Easing.OutCubic } }
        }
    }

    ParallelAnimation {
        id: enter
        NumberAnimation { target: c0; property: "opacity"; from: 0; to: 1; duration: Theme.normal; easing.type: Easing.OutCubic }
        NumberAnimation { target: t0; property: "y"; from: 16; to: 0; duration: Theme.normal; easing.type: Easing.OutCubic }
        SequentialAnimation {
            PauseAnimation { duration: 60 }
            ParallelAnimation {
                NumberAnimation { target: tabs; property: "opacity"; from: 0; to: 1; duration: Theme.normal; easing.type: Easing.OutCubic }
                NumberAnimation { target: t1; property: "y"; from: 16; to: 0; duration: Theme.normal; easing.type: Easing.OutCubic }
            }
        }
        SequentialAnimation {
            PauseAnimation { duration: 120 }
            ParallelAnimation {
                NumberAnimation { target: panel; property: "opacity"; from: 0; to: 1; duration: Theme.normal; easing.type: Easing.OutCubic }
                NumberAnimation { target: t2; property: "y"; from: 16; to: 0; duration: Theme.normal; easing.type: Easing.OutCubic }
            }
        }
    }

    // Crédito: nome do app e link do autor, no rodapé da coluna esquerda.
    Row {
        id: credit
        x: Theme.s5
        y: win.height - height - Theme.s5
        spacing: Theme.s1
        Text {
            text: backend.appName + " ·"
            font.family: Theme.family; font.pixelSize: Theme.sizeLabel
            color: Theme.text2
        }
        Text {
            id: creditLink
            text: backend.creditUrl.replace("https://", "")
            font.family: Theme.family; font.pixelSize: Theme.sizeLabel
            font.underline: creditMa.containsMouse || activeFocus
            color: creditMa.containsMouse || activeFocus ? Theme.accent : Theme.text2
            activeFocusOnTab: true
            Keys.onReturnPressed: Qt.openUrlExternally(backend.creditUrl)
            Keys.onSpacePressed: Qt.openUrlExternally(backend.creditUrl)
            Behavior on color { ColorAnimation { duration: Theme.d(Theme.fast) } }
            MouseArea {
                id: creditMa
                anchors.fill: parent
                anchors.margins: -6
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: Qt.openUrlExternally(backend.creditUrl)
            }
        }
    }

    Toast { id: toast }
}
