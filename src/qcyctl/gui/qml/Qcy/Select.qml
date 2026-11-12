import QtQuick
import QtQuick.Controls.Basic as C
import Qcy

// Caixa de seleção no tema do app.
C.ComboBox {
    id: root
    implicitHeight: Theme.control
    property string prefix: ""        // marca curta antes do texto (ex.: lado do fone)
    font.family: Theme.family
    font.pixelSize: Theme.sizeBody

    background: Rectangle {
        radius: Theme.radiusSm
        color: root.pressed ? Theme.surfacePress : (root.hovered ? Theme.surfaceHi : Theme.bg)
        border.width: 1
        border.color: root.popup.visible ? Theme.accent : Theme.edgeLight
        Behavior on color { ColorAnimation { duration: Theme.d(Theme.fast) } }
        FocusRing { show: root.visualFocus; r: Theme.radiusSm }
    }
    contentItem: Row {
        leftPadding: Theme.s3
        spacing: Theme.s2
        Text {
            visible: root.prefix !== ""
            anchors.verticalCenter: parent.verticalCenter
            text: root.prefix
            font.family: Theme.family; font.pixelSize: Theme.sizeLabel; font.weight: Font.DemiBold
            color: Theme.accent
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: root.width - parent.leftPadding - 28 - (root.prefix !== "" ? 16 : 0)
            text: root.displayText
            font: root.font
            color: Theme.text
            elide: Text.ElideRight
        }
    }
    indicator: Icon {
        x: root.width - width - Theme.s2
        y: (root.height - height) / 2
        width: 16; height: 16
        name: "chevron"
        rotation: root.popup.visible ? 180 : 0
        Behavior on rotation { NumberAnimation { duration: Theme.d(Theme.fast) } }
    }
    delegate: C.ItemDelegate {
        id: dlg
        required property int index
        required property var modelData
        width: root.popup.width - 8
        x: 4
        height: 34
        highlighted: root.highlightedIndex === index
        contentItem: Text {
            text: dlg.modelData[root.textRole] !== undefined ? dlg.modelData[root.textRole] : dlg.modelData
            font: root.font
            color: dlg.index === root.currentIndex ? Theme.accent : Theme.text
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            radius: Theme.radiusSm - 2
            color: dlg.highlighted ? Theme.surfaceHi : "transparent"
        }
    }
    popup: C.Popup {
        y: root.height + 4
        width: root.width
        implicitHeight: Math.min(contentItem.implicitHeight + 8, 320)
        padding: 4
        transformOrigin: C.Popup.Top
        enter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.d(Theme.fast) }
            NumberAnimation { property: "scale"; from: Theme.reduce ? 1 : 0.96; to: 1; duration: Theme.d(Theme.fast); easing.type: Easing.OutCubic }
        }
        exit: Transition {
            NumberAnimation { property: "opacity"; to: 0; duration: Theme.instant }
        }
        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: root.popup.visible ? root.delegateModel : null
            currentIndex: root.highlightedIndex
        }
        background: Rectangle {
            radius: Theme.radiusSm
            color: Theme.surfaceHi
            border.width: 1
            border.color: Theme.edgeLight
        }
    }
}
